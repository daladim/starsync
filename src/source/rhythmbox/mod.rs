//! Rhythmbox support
//!
//! This mostly uses mpris, is a generic protocol that exposes many music players over D-Bus
//! However, song ratings are not a standard, and are not exposed over mpris. We use Rhythmbox-specific functions for this part
//!
//! Note that Rhythmbox initially did not provide persistent IDs for tracks.
//! See https://gitlab.gnome.org/GNOME/rhythmbox/-/issues/2071 to tell which is the earliest version that does.

use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use std::path::PathBuf;
use std::num::NonZeroU8;

use dbus::blocking::{Connection, Proxy};
use dbus::arg::{PropMap, RefArg, Variant};
use log::{info, warn};

use self::rhythmdb::OrgGnomeRhythmbox3RhythmDB;

use super::{Source, Playlist, Rating, Track, TrackId, PlaylistId};


mod entry;
use entry::OrgFreedesktopDBusProperties;
mod mediaplayer;
use mediaplayer::{OrgMprisMediaPlayer2Player, OrgMprisMediaPlayer2Playlists};
mod playlists;
use playlists::OrgGnomeUPnPMediaContainer2;
mod rhythmdb;
mod playlistmanager;


const TIMEOUT: Duration = Duration::from_secs(1);

pub struct Rhythmbox {
    connection: Connection,
}

impl Rhythmbox {
    pub fn try_new() -> Option<Self> {
        let connection = match Connection::new_session() {
            Err(err) => {
                warn!("Unable to open D-Bus session ({err})");
                return None
            },
            Ok(c) => c,
        };
        let s = Self{ connection };

        if let Err(err) = s.player().metadata() {
            info!("Unable to find a Rhythmbox object: {err}");
            return None;
        }

        Some(s)
    }

    fn player(&self) -> Proxy<&Connection> {
        self
            .connection
            .with_proxy(
                "org.mpris.MediaPlayer2.rhythmbox",
                "/org/mpris/MediaPlayer2",
                TIMEOUT,
            )
    }
}

impl Source for Rhythmbox {
    fn name(&self) -> &str {
        "Rhythmbox"
    }

    fn playlists(&self) -> Result<Vec<Box<dyn Playlist>>, Box<dyn Error>> {
        let mut playlists = Vec::new();

        let rb_lists = self.player().get_playlists(0, u32::MAX, "", false)?;
        for data in rb_lists {
            match RhythmboxPlaylist::try_from(data) {
                Err(err) => warn!("Failed to parse playlist ({err})"),
                Ok(pl) => {
                    playlists.push(Box::new(pl) as Box<dyn Playlist>);
                }
            }
        }

        Ok(playlists)
    }

    fn playlist_by_name(&self, name: &str) -> Option<Box<dyn Playlist>> {
        self
            .player()
            .get_playlists(0, u32::MAX, "", false)
            .ok()?
            .into_iter()
            .find(|(_pl_path, pl_name, _other)| pl_name.as_str() == name)
            .and_then(|data| match RhythmboxPlaylist::try_from(data) {
                Err(err) => {
                    warn!("Unable to parse playlist ({err})");
                    None
                },
                Ok(pl) => Some(Box::new(pl) as Box<dyn Playlist>)
            })
    }

    fn playlist_by_id(&self, id: &PlaylistId) -> Option<Box<dyn Playlist>> {
        let name = match id {
            PlaylistId::Name(s) => s,
            _ => {
                warn!("Invalid type ({id:?}) for playlist ID.");
                return None;
            }
        };

        self.playlist_by_name(name)
    }

    fn track_by_id(&self, id: TrackId) -> Option<Box<dyn Track>> {
        match RhythmboxEntry::try_from_id(id) {
            Ok(song) => Some(Box::new(song) as Box<dyn Track>),
            Err(err) => {
                warn!("Unable to fetch track from ID {id:?} ({err})");
                None
            }
        }
    }
}



pub struct RhythmboxPlaylist {
    name: String,
    /// This is a memory address, and will not be valid again after Rhythmbox is restarted
    temp_address: u64,
}

impl RhythmboxPlaylist {
    fn try_from(data: (dbus::Path<'static>, String, String)) -> Result<Self, Box<dyn Error>> {
        let (path, name) = (data.0, data.1);

        // path is e.g. /org/gnome/Rhythmbox3/Playlist/0x55abf9bd4b70
        // Let's extract its ID
        let hex_id = path
            .split("/")
            .last()
            .and_then(|part| part.strip_prefix("0x"))
            .ok_or("Unable to extract playlist ID")?;

        let temp_address: u64 = u64::from_str_radix(hex_id, 16)?;

        Ok(Self{ name, temp_address })
    }

    /// This path is only valable for this runtime session of Rhythmbox
    fn runtime_dbus_path(&self) -> String {
        format!("/org/gnome/UPnP/MediaServer2/Playlists/{}", self.temp_address)
    }

    fn entries(&self) -> Result<Vec<RhythmboxEntry>, Box<dyn Error>> {
        let path = self.runtime_dbus_path();

        Ok(Connection::new_session()?
            .with_proxy("org.mpris.MediaPlayer2.rhythmbox", &path, TIMEOUT)
            .list_items(0, u32::MAX, vec!["Path"])?
            .into_iter()
            .filter_map(|data| match RhythmboxEntry::try_from_path_propmap(data) {
                Ok(e) => Some(e),
                Err(err) => {
                    warn!("Failed to parse entry {path:?} ({err}).");
                    None
                }
            })
            .collect())
    }

    fn remove_file(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let connection = Connection::new_session()?;
        let proxy = connection.with_proxy("org.mpris.MediaPlayer2.rhythmbox", "/org/gnome/Rhythmbox3/PlaylistManager", TIMEOUT);

        // TODO: what happens if a playlist has multiple instances of the same file?
        //       there's probably a possible bug here. But I may not have a choice, as this is the only available API to remove an item from a playlist :-(
        Ok(playlistmanager::OrgGnomeRhythmbox3PlaylistManager::remove_from_playlist(&proxy, &self.name, url)?)
    }

    fn add_file(&self, url: &str) -> Result<(), Box<dyn Error>> {
        let connection = Connection::new_session()?;
        let proxy = connection.with_proxy("org.mpris.MediaPlayer2.rhythmbox", "/org/gnome/Rhythmbox3/PlaylistManager", TIMEOUT);

        Ok(playlistmanager::OrgGnomeRhythmbox3PlaylistManager::add_to_playlist(&proxy, &self.name, url)?)
    }
}

impl Playlist for RhythmboxPlaylist {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn tracks(&self) -> Result<Vec<Box<dyn Track>>, Box<dyn Error>> {
        Ok(self.entries()?
            .into_iter()
            .map(|entry| Box::new(entry) as Box<dyn Track>)
            .collect())
    }

    fn id(&self) -> PlaylistId {
        PlaylistId::Name(self.name.clone())
    }

    /// Change the content of this playlist.
    ///
    /// This may merely re-order songs, but also remove or add songs.
    fn change_contents_to(&self, new_content: &[TrackId]) -> Result<(), Box<dyn Error>> {
        change_contents_to_inner(self, new_content)
    }
}


fn change_contents_to_inner(playlist: &RhythmboxPlaylist, new_content: &[TrackId]) -> Result<(), Box<dyn Error>> {
    let get_i_th_track = |i| {
        let entries = playlist.entries()?;
        if i < entries.len() {
            playlist
                .entries()
                .map(|mut tracks| Some(tracks.swap_remove(i)))
        } else {
            Ok(None)
        }
    };

    let mut i = 0;
    for required_id in new_content {
        i += 1;
        loop {
            match get_i_th_track(i) {
                Ok(Some(i_th_track)) => {
                    if i_th_track.persistent_id() == required_id {
                        // Both lists match up to index i.
                        // Let's proceed to the next required track
                        log::trace!("Right track {:?} ({:?})", i_th_track.persistent_id(), i_th_track.name());
                        break;
                    } else {
                        // Let's remove the non-matching track. Maybe the next one will
                        log::trace!("Deleting {:?} ({:?})", i_th_track.persistent_id(), i_th_track.name());
                        playlist.remove_file(i_th_track.encoded_file_path())?;
                    }
                },
                Ok(None) => {
                    // The Rhythmbox playlist has no more tracks. Let's add the one that is required.
                    let required_track = RhythmboxEntry::try_from_id(*required_id)?;

                    log::trace!("Adding {:?} ({:?})", required_id, required_track.name());
                    playlist.add_file(&required_track.encoded_file_path())?;
                    break;
                },
                Err(err) => {
                    Err(err)?
                }
            }
        }
    }

    // The head of the Rhythmbox playlist matches the requirements.
    // Are there remaining tracks to remove?
    i += 1;
    loop {
        match get_i_th_track(i) {
            Ok(Some(extra_track)) => playlist.remove_file(extra_track.encoded_file_path())?,
            Ok(None) => break,
            Err(err) => Err(err)?,
        }
    }

    Ok(())
}



pub struct RhythmboxEntry {
    display_name: String,
    entry_id: TrackId,
    file_path: PathBuf,
    encoded_file_path: String,
    rating: Rating,
}

impl RhythmboxEntry {
    fn persistent_id(&self) -> &TrackId {
        &self.entry_id
    }

    fn encoded_file_path(&self) -> &str {
        &self.encoded_file_path
    }

    pub fn try_from_path_propmap(data: PropMap) -> Result<Self, Box<dyn Error>> {
        let dbus_path = data
            .get("Path")
            .and_then(|var| var.as_str())
            .ok_or(format!("No D-Bus path is available for song {data:?}"))?
            .to_string();

        let entry_id = TrackId(dbus_path
            .strip_prefix("/org/gnome/UPnP/MediaServer2/Entry/")
            .ok_or(format!("Invalid D-Bus path ({dbus_path})"))
            .and_then(|s| s.parse()
                .map_err(|_| String::from("Unable to parse song ID for {dbus_path}"))
            )?);

        Self::try_from_id(entry_id)
    }

    pub fn try_from_id(entry_id: TrackId) -> Result<Self, Box<dyn Error>> {
        let dbus_path = format!("/org/gnome/UPnP/MediaServer2/Entry/{}", entry_id.0);

        let display_name = Connection::new_session()?
            .with_proxy("org.mpris.MediaPlayer2.rhythmbox", &dbus_path, TIMEOUT)
            .get("org.gnome.UPnP.MediaObject2", "DisplayName")?
            .as_str()
            .unwrap_or("Unknown")
            .to_string();

        let urls = Connection::new_session()?
            .with_proxy("org.mpris.MediaPlayer2.rhythmbox", &dbus_path, TIMEOUT)
            .get("org.gnome.UPnP.MediaItem2", "URLs")?;
        let encoded_file_path = urls
            // for some reason, this is a Variant that contains an array of arrays...
            .as_iter()
            .and_then(|mut i| i.next())
            .and_then(|v| v.as_iter())
            .and_then(|mut i| i.next())
            .and_then(|s| s.as_str())
            .ok_or(format!("No file path is available for song {display_name}"))?
            .to_string();

        let decoded_file_path = urlencoding::decode(&encoded_file_path)?;
        let file_path = PathBuf::from(decoded_file_path
            .strip_prefix("file://")
            .unwrap_or(&decoded_file_path));

        let rating = Connection::new_session()?
            .with_proxy("org.mpris.MediaPlayer2.rhythmbox", "/org/gnome/Rhythmbox3/RhythmDB", TIMEOUT)
            .get_entry_properties(&encoded_file_path)?
            .get("rating")
            .and_then(|r| r.as_f64())
            .map(|f| f as u8)
            .and_then(|u| NonZeroU8::new(u));

        Ok(Self { display_name, entry_id, file_path, encoded_file_path, rating })
    }
}

impl Track for RhythmboxEntry {
    fn name(&self) -> String {
        self.display_name.clone()
    }

    fn id(&self) -> TrackId {
        self.entry_id
    }

    fn absolute_path(&self) -> Result<PathBuf, Box<dyn Error>> {
        Ok(self.file_path.clone())
    }

    fn rating(&self, _use_computed_ratings: bool) -> Rating {
        self.rating
    }

    fn set_rating(&self, new_rating: Rating) -> Result<(), Box<dyn Error>> {
        let new_rating = new_rating.map(|nzu| nzu.get()).unwrap_or(0);
        let mut items = HashMap::new();
        items.insert("rating".to_string(), Variant(Box::new(new_rating as f64) as Box<dyn RefArg>));

        Connection::new_session()?
            .with_proxy("org.mpris.MediaPlayer2.rhythmbox", "/org/gnome/Rhythmbox3/RhythmDB", TIMEOUT)
            .set_entry_properties(&self.encoded_file_path, items)?;

        Ok(())
    }

    fn file_size(&self) -> Result<usize, Box<dyn Error>> {
        let md = std::fs::metadata(&self.file_path)?;
        Ok(usize::try_from(md.len())?)
    }
}
