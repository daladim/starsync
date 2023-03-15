//! Functions to sync a device against its source
//!
//! This process is supposed to be somehow interactive, because it may include some prompts to the user.<br/>
//! This is why its API is split into several structs, supposed to be used consecutively.<br/>
//! A sync process starts by building a [`SyncBuilder`].

use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::error::Error;
use std::collections::{HashSet, HashMap};
use std::sync::mpsc::{Sender, Receiver};

use crate::device::{Device, Folder};
use crate::device::m3u::M3u;
use crate::source::{Source, ItemId, Rating};
use crate::config::Config;
use crate::utils::current_hostname;

pub mod status;
use status::Message;
use status::Progress;

mod info;
pub use info::SyncInfo;

mod utils;
use utils::{FileSet, FileData, RequestedPlaylistKind, ActualPlaylistKind};
use utils::favourites_playlist_name;

/// How many warnings have been issued
pub type Warnings = usize;

#[derive(thiserror::Error, Debug)]
pub enum SyncError {
    #[error("Source {0} not found")]
    SourceNotFound(String),
    #[error("Device {0} not found")]
    DeviceNotFound(String),
    #[error("Error when reading from the device")]
    DeviceReadError,
    #[error("This device is not inited")]
    NotInited,
    #[error("Some sanity checks have failed")]
    SanityChecks,
    #[error("Scanning the computer for songs has failed: {0}")]
    SongScanningFailed(String),
    #[error("Syncing files to the device failed: {0}")]
    SyncingFilesFailed(String),
    #[error("Pushing updated playlists to the device failed: {0}")]
    PushingPlaylistsFailed(String),
    #[error("Pushing info about the current sync session into the device has failed: {0}")]
    UpdateSyncInfoFailed(String),
    // TODO: this could be supported after all, we'll just have to add one level of folders, with arbitrary names, one for each set of common ancestors
    #[error("Files have no common ancestor, there is no way to know how they should be saved into the device")]
    NoCommonAncestor,
}


pub struct SyncManager {
    device: Box<dyn Device>,
    source: Box<dyn Source>,
    config: Config,
    previous_sync_infos: Option<SyncInfo>,
}

impl SyncManager {
    /// Initiate a sync with a given device.
    ///
    /// This will fetch the config stored on this device
    pub fn with_device(device_name: &str) -> Result<Self, SyncError> {
        // Get the device
        let device = crate::device::get(device_name).ok_or_else(|| SyncError::DeviceNotFound(device_name.to_string()))?;
        if device.starsync_folder().is_none() {
            return Err(SyncError::NotInited);
        }

        // Get the config
        let config = device.config().ok_or(SyncError::NotInited)?;

        // Get info from the latest sync
        let latest_info = device.previous_sync_infos();

        Self::with_options(device, config, latest_info)
    }

    /// Initiate a sync with a given device, using a specific config
    fn with_options(device: Box<dyn Device>, config: Config, previous_sync_infos: Option<SyncInfo>) -> Result<Self, SyncError> {
        // Get the source
        let source_name = config.source();
        let source = crate::source::get(source_name).ok_or_else(|| SyncError::SourceNotFound(source_name.to_string()))?;

        Ok( Self{device, source, config, previous_sync_infos} )
    }

    /// Performs some sanity check, and returns their result
    ///
    /// # Workflow
    ///
    /// This is usually supposed to be called in a background thread, while the main thread could show the sync progress.<br/>
    /// Because I have no clue whether the underlying COM objects are thread-safe, it is better to create and use them only in one thread at a time.<br/>
    /// But actually, the compiler is picky about creating them on one thread _then_ moving them to another thread, so let's create _and_ use them on the same thread.<br/>
    /// This means you'll have to call [`Self::start_sync`] on the background thread.
    ///
    /// But since the sync may incur some interactivity with the user, you'll have to provide channels to get and send info to this background thread.<br/>
    /// That's why there are:
    /// * a `outbound` mpsc, that (at least currently?) could have been a `Oneshot`, that will receive a [`SyncValidator`]
    /// * an `inbound` mpsc, that (at least currently?) could have been a `Oneshot`, that will receive the acknowledged [`SyncValidator`] to start the actual sync
    /// * a `progress` mpsc, that will receive messages as-they-happen and that describe the progress of the sync.
    ///
    /// # Errors
    ///
    /// Only fatal errors are reported in the `Err` return value.<br/>
    /// Warnings are passed into the [`status::Sender`], and are counted in the `Ok(Warnings)` return value.
    pub fn start_sync(
        &self,
        status_tx: status::Sender,
        outbound: Sender<SyncValidator>,
        inbound: Receiver<SyncValidator>
    ) -> Result<Warnings, SyncError> {
        let validator = SyncValidator::build(self.previous_sync_infos.as_ref());
        outbound.send(validator).expect("transmission to be possible");

        let acknowledged_validator = inbound.recv().expect("sender end not to disconnect");
        if acknowledged_validator.is_valid() {
            self.sync_inner(&status_tx)?;
            Ok(status_tx.warnings_count())
        } else {
            Err(SyncError::SanityChecks)
        }
    }

    /* not pub, see `start_sync` instead */ fn sync_inner (&self, status_tx: &status::Sender) -> Result<(), SyncError> {
        status_tx.send_progress(Progress::Started);
        let previous_sync_info = self.device.previous_sync_infos();

        let files_on_device = files_on_device(status_tx, self.device.as_ref())?;

        // Reverse sync
        reverse_sync_playlists(status_tx, &previous_sync_info, self.source.as_ref(), self.device.as_ref());

        // Reverse sync for ratings
        if self.config.include_stars() {
            reverse_sync_ratings(status_tx, &previous_sync_info, &files_on_device, self.source.as_ref(), self.device.as_ref());
        }

        // Build the list of files that should be on the device
        let file_set = required_files(status_tx, self.source.as_ref(), &self.config)
            .map_err(|err| SyncError::SongScanningFailed(err.to_string()))?;

        // Push and delete files
        sync_files(status_tx, &file_set, &files_on_device, self.device.as_ref())
            .map_err(|err| SyncError::SyncingFilesFailed(err.to_string()))?;

        // Push playlists
        let playlists = update_playlists(status_tx, self.source.as_ref(), self.device.as_ref(), &self.config, &file_set.common_ancestor)
            .map_err(|err| SyncError::PushingPlaylistsFailed(err.to_string()))?;

        // Push made-up star playlists
        if self.config.include_stars() {
            push_star_playlists(status_tx, self.device.as_ref(), &file_set);
        }

        // Update the last sync info
        update_sync_info(self.device.as_ref(), file_set, playlists)
            .map_err(|err| SyncError::UpdateSyncInfoFailed(err.to_string()))?;

        status_tx.send_progress(Progress::Done);

        Ok(())
    }
}

/// The results of a sanity check.
///
/// In case some checks failed, it is OK to acknowledge them by setting them to `true`, but that's a good idea
/// to do so only after having prompted the user for confirmation.
///
/// The sync will only start when all these checks are set (or overridden) to `true`.
pub struct SyncValidator {
    /// In case we are not attempting to sync with the same computer as last time, this will contain the previous and the current hostnames
    pub last_sync_computer_mismatch: Option<(String, String)>,
}

impl SyncValidator {
    fn build(previous_sync_infos: Option<&SyncInfo>) -> Self {
        let last_sync_computer_mismatch = previous_sync_infos.and_then(|psi| {
            let chn = current_hostname();
            if psi.hostname() != chn {
                Some((psi.hostname().to_string(), chn))
            } else {
                None
            }
        });

        Self {
            last_sync_computer_mismatch
        }
    }

    fn is_valid(&self) -> bool {
        self.last_sync_computer_mismatch.is_none()
    }
}

fn m3u_to_song_ids(status_tx: &status::Sender, playlist: M3u, previous_sync_info: &SyncInfo) -> Vec<ItemId> {
    playlist
        .paths()
        .filter_map(|path| previous_sync_info
            .id_for_relative_path(path.strip_prefix(crate::device::MUSIC_FOLDER_NAME).unwrap_or(path))
            .or_else(|| {
                status_tx.send_warning(format!("Unable to get ID for song at path '{}' on device.", path.display()));
                None
            })
        )
        .collect()
}

fn reverse_sync_playlists(status_tx: &status::Sender, previous_sync_info: &Option<SyncInfo>, source: &dyn Source, device: &dyn Device) {
    status_tx.send_progress(Progress::ReverseSyncPlaylists);

    let previous_sync_info = match previous_sync_info {
        Some(psi) => psi,
        None => {
            // In case there was no previous sync, there is nothing to reverse sync.
            status_tx.send_info("This seems to be the first time this device is synced. Not performing reverse sync for playlists");
            return;
        }
    };

    let playlists_on_device = match playlists_on_device(status_tx, RequestedPlaylistKind::Regular, device, previous_sync_info) {
        Err(err) => {
            status_tx.send_warning(format!("Unable to get playlists from device: {}", err));
            return;
        },
        Ok(lists) => lists,
    };

    // Convert file paths to song IDs
    let playlists_on_device: HashMap<String, Vec<ItemId>> = playlists_on_device.into_iter()
        .map(|(name, playlist)|
            (name, m3u_to_song_ids(status_tx, playlist, previous_sync_info))
        )
        .collect();

    for (playlist_name_on_device, device_song_ids) in playlists_on_device {
        match previous_sync_info.playlist(&playlist_name_on_device) {
            None => {
                status_tx.send_warning(format!("Unable to get info about the last sync of playlist '{}'.", playlist_name_on_device));
            },
            Some((playlist_id, ancestor_song_ids)) => {
                if let Err(err) = reverse_sync_playlist(status_tx, source, &playlist_name_on_device, *playlist_id, ancestor_song_ids, &device_song_ids) {
                    status_tx.send_warning(format!("Unable to reverse sync playlist '{}': {}", playlist_name_on_device, err));
                }
            }
        }
    }
}

fn reverse_sync_ratings(
    status_tx: &status::Sender,
    previous_sync_info: &Option<SyncInfo>,
    files_on_device: &HashSet<PathBuf>,
    source: &dyn Source,
    device: &dyn Device,
) {
    //
    //
    //
    //
    // TODO: review all the list iterations and collecting and conversions
    //

    status_tx.send_progress(Progress::ReverseSyncRatings);

    let previous_sync_info = match previous_sync_info {
        Some(psi) => psi,
        None => {
            // In case there was no previous sync, there is nothing to reverse sync.
            status_tx.send_info("This seems to be the first time this device is synced. Not performing reverse sync for ratings");
            return;
        }
    };

    let rating_playlists_on_device = match playlists_on_device(status_tx, RequestedPlaylistKind::Ratings, device, previous_sync_info) {
        Err(err) => {
            status_tx.send_warning(format!("Unable to get ratings from device: {}", err));
            return;
        },
        Ok(lists) => lists,
    };

    // Get the IDs of every file on the device
    // This will be useful when detecting track that have no rating
    let mut no_ratings: HashSet<ItemId> = files_on_device.iter().filter_map(|path| previous_sync_info.id_for_relative_path(path)).collect();

    // Convert:
    // * playlist name to rating value
    // * file paths to song IDs
    let mut ratings_on_device: HashMap<Option<u8>, HashSet<ItemId>> = HashMap::new();

    for (name, m3u) in rating_playlists_on_device {
        match ActualPlaylistKind::classify(&name).stars() {
            None => {
                status_tx.send_warning(format!("Unexpected non-ratings list '{}'", name));
            }
            Some(stars) => {
                let ids_with_this_rating = m3u_to_song_ids(status_tx, m3u, previous_sync_info).iter().copied().collect();

                // Remove tracks that are rated from no_ratings, so that it eventually lists tracks...that have no rating
                for rated_id in &ids_with_this_rating {
                    if no_ratings.remove(rated_id) == false {
                        status_tx.send_warning(format!("Song with ID {:x?} is rated, but it does not look like it is present on the device", rated_id));
                    }
                }

                ratings_on_device.insert(
                    Some(stars),
                    ids_with_this_rating,
                );
            },
        };
    }

    // Add the songs that have no rating
    ratings_on_device.insert(None, no_ratings);

    // Check which track has changed its rating
    for (rating_on_device, list) in ratings_on_device {
        for track_id in list {
            let rating_at_previous_sync = previous_sync_info.rating_for_id(track_id);
            if rating_at_previous_sync != rating_on_device {
                // This song has changed its rating on the device.
                // Has it changed on the source as well?
                match source.track_by_id(track_id) {
                    None => status_tx.send_warning(format!("The rating of track {:x?} has changed on the device, but it has been removed from the source", track_id)),
                    Some(track) => {
                        let rating_on_source = track.rating();
                        let track_name = previous_sync_info
                            .path_for_id(track_id)
                            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                            .unwrap_or("<unknown>".to_string());

                        if rating_on_source != rating_at_previous_sync {
                            // That's a conflict
                            status_tx.send_info(format!("Song {:?} has changed its rating on both the source and the device. That's a conflict, let the source win.", track_name));
                        } else {
                            // We are cleared to update the rating on the source
                            status_tx.send(Message::UpdatingSongRatingIntoSource{ track_name: track_name.clone(), new_rating: rating_on_device });
                            if let Err(err) = track.set_rating(rating_on_device) {
                                status_tx.send_warning(format!("Unable to update rating for track '{}' (to {:?} stars): {}", &track_name, rating_on_device, err));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn reverse_sync_playlist(status_tx: &status::Sender, source: &dyn Source, playlist_name: &str, playlist_id: ItemId, ancestor_song_ids: &[ItemId], device_song_ids: &[ItemId]) -> Result<(), Box<dyn Error>> {
    status_tx.send(Message::ReverseSyncPlaylist(playlist_name.to_string()));

    let local_playlist = source.playlist_by_id(playlist_id).ok_or("No such playlist")?;
    let local_song_ids: Vec<ItemId> = local_playlist
        .tracks()?
        .iter()
        .map(|track| track.id())
        .collect();

    // In case all playlists are the same, let's not bother doing a 3-way merge
    if device_song_ids == local_song_ids {
        status_tx.send_info(format!("Playlist {} has not been modified, skipping it.", playlist_name));
        return Ok(());
    }

    let new_song_order = diffy::merge_custom(ancestor_song_ids, &local_song_ids, device_song_ids)?;
    let owned_ids: Vec<ItemId> = new_song_order.iter().map(|id| **id).collect();
    status_tx.send(Message::UpdatingPlaylistIntoSource{new_content: owned_ids.to_vec()});
    if let Err(err) = local_playlist.change_contents_to(&owned_ids) {
        status_tx.send_warning(format!("Unable to update the contents of playlist {}: {}", playlist_name, err));
    }

    Ok(())
}

fn required_files(status_tx: &status::Sender, source: &dyn Source, config: &Config) -> Result<FileSet, Box<dyn Error>> {
    status_tx.send_progress(Progress::ListingFiles);

    let mut total_size = 0;
    let mut files_data = HashMap::new();

    for playlist_name in config.playlists() {
        match source.playlist_by_name(playlist_name) {
            None => status_tx.send_warning(format!("Unable to find playlist '{}'", playlist_name)),
            Some(list) => {
                match list.tracks() {
                    Err(err) => status_tx.send_warning(format!("Unable to list tracks for playlist '{}': {}", list.name(), err)),
                    Ok(tracks) => {
                        for track in tracks {
                            match track.absolute_path() {
                                Err(err) => status_tx.send_warning(format!("Unable to get path for song '{}': {}", track.name(), err)),
                                Ok(path) => {
                                    let file_size = match track.file_size() {
                                        Err(err) => {
                                            status_tx.send_warning(format!("Unable to get file size for song '{}': {}", track.name(), err));
                                            0
                                        },
                                        Ok(size) => size,
                                    };

                                    let rating = track.rating();

                                    if files_data.insert(
                                        path.clone(),
                                        FileData{ file_size, id: track.id(), rating }
                                    ).is_some() {
                                        // We've already kept track of this file, as it is in duplicate playlists.
                                        // We must not count its size twice.
                                        // No need to add it to the list of IDs either
                                    } else {
                                        total_size += file_size;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Get the common ancestor for all these files
    let common_ancestor = crate::common_path::common_path_all(files_data.keys()).ok_or(SyncError::NoCommonAncestor)?;

    Ok(FileSet{ common_ancestor, files_data, total_size })
}

fn sync_files(status_tx: &status::Sender, file_set: &FileSet, files_on_device: &HashSet<PathBuf>, device: &dyn Device) -> Result<(), SyncError> {
    let FileSet{ files_data, common_ancestor, .. } = file_set;

    // What files should there be on the device?
    let relative_files: HashSet<PathBuf> = files_data
        .keys()
        .filter_map(|path| path
            .strip_prefix(common_ancestor)
            .map(|path| path.to_owned())
            .map_err(|_err| status_tx.send_warning(format!("File '{:?}' is not a child of the root folder '{:?}'. Ignoring this file", path, common_ancestor)))
            .ok()
        )
        .collect();

    // What files are there already?
    let files_to_remove = files_on_device.difference(&relative_files);
    let files_to_push = relative_files.difference(files_on_device);

    // Actually sync files
    status_tx.send_progress(Progress::SyncingFiles);
    let device_root = device.music_folder().ok_or(SyncError::DeviceReadError)?;
    for file_to_remove in files_to_remove {
        status_tx.send(Message::RemovingFile(file_to_remove.display().to_string()));
        if let Err(err) = device_root
            .file_at(file_to_remove)
            .and_then(|f| f.delete())
        {
            status_tx.send_warning(format!("Unable to remove file at {}: {}", file_to_remove.display(), err))
        }
    }

    for file_to_push in files_to_push {
        // TODO: optim: have a single data structure (hashmap) that contains both absolute and relative file, so that there is no need to re-build it here
        status_tx.send(Message::PushingFile(file_to_push.display().to_string()));
        let local_absolute_path = common_ancestor.join(file_to_push);
        if let Err(err) = device.push_music_file(&local_absolute_path, file_to_push) {
            status_tx.send_warning(format!("Unable to push file {}: {}", file_to_push.display(), err));
        }
    }

    // TODO: remove empty folders

    Ok(())
}

fn playlists_on_device(status_tx: &status::Sender, requested_kind: RequestedPlaylistKind, device: &dyn Device, previous_sync_info: &SyncInfo) -> Result<HashMap<String, M3u>, SyncError> {
    let playlists_folder = device.starsync_folder().ok_or(SyncError::DeviceReadError)?;
    let mut playlists_on_device = HashMap::new();

    for file in playlists_folder.files().map_err(|_err| SyncError::DeviceReadError)? {
        let file_path = file.path();
        if file_path.extension() == Some(OsStr::new("m3u")) {
            let file_name = file_path
                .file_name()
                .map(|osstr| osstr.to_string_lossy().to_string())
                .unwrap_or_else(|| "<no name>".to_string());

            // Are we interested in processing this list?
            let actual_kind = ActualPlaylistKind::classify(&file_name);
            match (requested_kind, actual_kind) {
                (RequestedPlaylistKind::Regular, ActualPlaylistKind::Regular(_)) => {
                    if previous_sync_info.has_playlist_file_name(&file_name) == false {
                        // We're not supposed to sync this playlist. Ignore it.
                        status_tx.send_info(format!("File {} exists, but the playlist is ignored because it is not selected in the config file.", file_path.display()));
                        continue;
                    }
                },
                (RequestedPlaylistKind::Regular, ActualPlaylistKind::Ratings(_)) => {
                    // No need to display anything, we can just silently ignore it
                    continue;
                },
                (RequestedPlaylistKind::Ratings, ActualPlaylistKind::Regular(_)) => {
                    // No need to display anything, we can just silently ignore it
                    continue;
                },
                (RequestedPlaylistKind::Ratings, ActualPlaylistKind::Ratings(_)) => {
                    // Do nothing and process it
                },
            }

            status_tx.send(Message::RetrievingDevicePlaylist(file_name.to_string()));
            match file.get_reader() {
                Err(err) => status_tx.send_warning(format!("Unable to get playlist file '{}' from device: {}", file_path.display(), err)),
                Ok(m3u_reader) => {
                    let m3u_playlist = M3u::parse(m3u_reader);
                    if let Some(_old_value) = playlists_on_device.insert(file_name, m3u_playlist) {
                        status_tx.send_warning(format!("Multiple playlists '{}' found on device", file_path.display()));
                    }
                }
            }
        }
    }

    Ok(playlists_on_device)
}

fn files_on_device(status_tx: &status::Sender, device: &dyn Device) -> Result<HashSet<PathBuf>, SyncError> {
    // Maybe this could be speeded up by just taking the values from the last_sync_info.
    // However, scanning the actual folders ensures we are robust to (more-or-less) accidental file deletions.

    let music_folder = device.music_folder().ok_or(SyncError::DeviceReadError)?;
    let root_folder_path = music_folder.path();
    let mut files_on_device = HashSet::new();

    populate_device_files(status_tx, root_folder_path, &mut files_on_device, music_folder.as_ref());

    Ok(files_on_device)
}

fn populate_device_files(status_tx: &status::Sender, root_folder_path: &Path, files_on_device: &mut HashSet<PathBuf>, current_folder: &dyn Folder) {
    match current_folder.files() {
        Err(err) => status_tx.send_warning(format!("Unable to list files from folder '{:?}': {}", current_folder.path(), err)),
        Ok(files) => {
            for file in files {
                let full_path = file.path();
                match file.path().strip_prefix(root_folder_path) {
                    Err(_err) => status_tx.send_warning(format!("Found a file ({:?}) that is not included in the root folder {:?}", full_path, root_folder_path)),
                    Ok(rel) => {
                        files_on_device.insert(rel.to_owned());
                    }
                }
            }
        }
    }

    match current_folder.sub_folders() {
        Err(err) => status_tx.send_warning(format!("Unable to list folders from folder '{:?}': {}", current_folder.path(), err)),
        Ok(folders) => {
            for folder in folders {
                populate_device_files(status_tx, root_folder_path, files_on_device, folder.as_ref());
            }
        }
    }
}


fn update_playlists(status_tx: &status::Sender, source: &dyn Source, device: &dyn Device, config: &Config, common_ancestor: &Path) -> Result<HashMap<String, (ItemId, Vec<ItemId>)>, SyncError> {
    status_tx.send_progress(Progress::PushingPlaylists);
    let main_folder = device.starsync_folder().ok_or(SyncError::DeviceReadError)?;

    // Remove previous playlists
    if let Err(err) = remove_current_playlists(status_tx, main_folder.as_ref()) {
        status_tx.send_warning(format!("Unable to remove playlists: {}", err));
    }

    // Push updated playlists
    let playlists = push_playlists(status_tx, device, source, config, common_ancestor);
    Ok(playlists)
}

fn remove_current_playlists(status_tx: &status::Sender, main_folder: &dyn Folder) -> Result<(), SyncError> {
    let m3u_extension = OsStr::new("m3u");

    for file in main_folder.files().map_err(|_| SyncError::DeviceReadError)? {
        if file.path().extension() == Some(m3u_extension) {
            status_tx.send(Message::RemovingPlaylist(file.path().display().to_string()));
            if let Err(err) = file.delete() {
                status_tx.send_warning(format!("Unable to delete {}: {}", file.path().display(), err));
            }
        }
    }

    Ok(())
}

fn push_playlists(status_tx: &status::Sender, device: &dyn Device, source: &dyn Source, config: &Config, common_ancestor: &Path) -> HashMap<String, (ItemId, Vec<ItemId>)> {
    let mut pushed_playlists = HashMap::new();

    for playlist_name in config.playlists() {
        match source.playlist_by_name(playlist_name) {
            None => status_tx.send_warning(format!("Unable to get local playlist '{}'", playlist_name)),
            Some(list) => {
                // Push an M3U file into the device
                match list.to_m3u(common_ancestor, Path::new(crate::device::MUSIC_FOLDER_NAME)) {
                    Err(err) => status_tx.send_warning(format!("Unable to generate m3u file for playlist '{}': {}", playlist_name, err)),
                    Ok(m3u_content) => {
                        let device_relative_path = list.suitable_filename();
                        status_tx.send(Message::PushingPlaylist(playlist_name.to_string()));
                        if let Err(err) = device.push_playlist(&m3u_content, &device_relative_path) {
                            status_tx.send_warning(format!("Unable to push m3u file for playlist '{}': {}", playlist_name, err));
                        }
                    }
                }

                // Populate the list of pushed playlists
                let playlist_id = list.id();
                match list.tracks() {
                    Err(err) => status_tx.send_warning(format!("Unable to get tracks from playlist '{}': {}", playlist_name, err)),
                    Ok(tracks) => {
                        let song_ids = tracks
                            .iter()
                            .map(|track| track.id())
                            .collect();

                        if let Some(_old_entry) = pushed_playlists.insert(
                            list.suitable_filename().to_string_lossy().to_string(),
                            (playlist_id, song_ids)
                        ) {
                            status_tx.send_warning(format!("Duplicate playlists named '{}'", playlist_name));
                        }
                    }
                }
            }
        }
    }

    pushed_playlists
}

fn push_star_playlists(status_tx: &status::Sender, device: &dyn Device, file_set: &FileSet) {
    for (rating, songs) in file_set.song_paths_by_rating().iter() {
        match crate::source::create_m3u(
            songs.iter(),
            &file_set.common_ancestor,
            Path::new(crate::device::MUSIC_FOLDER_NAME))
        {
            Err(err) => status_tx.send_warning(format!("Unable to generate m3u file for songs rated {} stars: {}", rating, err)),
            Ok(m3u_content) => {
                let playlist_file_name = favourites_playlist_name(*rating);
                status_tx.send(Message::PushingPlaylist(playlist_file_name.to_string_lossy().to_string()));
                if let Err(err) = device.push_playlist(&m3u_content, &playlist_file_name) {
                    status_tx.send_warning(format!("Unable to push m3u file for rating playlist '{}': {}", playlist_file_name.display(), err));
                }
            }
        }
    }
}

fn update_sync_info(device: &dyn Device, file_set: FileSet, playlists: HashMap<String, (ItemId, Vec<ItemId>)>) -> Result<(), Box<dyn Error>> {
    let FileSet{ common_ancestor, files_data, .. } = file_set;
    let song_data_to_serialize = files_data
        .iter()
        .map(|(path, FileData{id, rating, ..})|
            (path.clone(), (*id, *rating)))
        .collect();
    let sync_info = SyncInfo::new(
        common_ancestor,
        song_data_to_serialize,
        playlists,
    );

    device.push_sync_infos(&sync_info)
}
