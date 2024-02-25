use std::error::Error;
use std::path::PathBuf;

use itunes_com::sys::ITRatingKind;
use itunes_com::wrappers::iTunes;
use itunes_com::wrappers::UserPlaylist as ITUserPlaylist;
use itunes_com::wrappers::Track as ITTrack;
use itunes_com::wrappers::IITObjectWrapper;
use itunes_com::wrappers::IITPlaylistWrapper;
use itunes_com::wrappers::IITTrackWrapper;
use itunes_com::wrappers::ITunesRelatedObject;
use itunes_com::wrappers::Iterable;

use super::{Source, Playlist, Track, ItemId};

pub struct ITunes {
    inner: iTunes,
}

impl ITunes {
    pub fn try_new() -> Option<Self> {
        iTunes::new().ok().map(|inner| Self{ inner })
    }
}

impl Source for ITunes {
    fn name(&self) -> &str {
        "iTunes"
    }

    fn playlists(&self) -> Result<Vec<Box<dyn Playlist>>, Box<dyn Error>> {
        Ok(self.inner.LibrarySource()?
            .Playlists()?
            .iter()?
            // it turns out thre is a "Music" user playlist, so there is no need grabbing also the LibraryPlaylist in this filter
            .filter_map(|list| list.as_user_playlist())
            .map(|list| Box::new(list) as Box<dyn Playlist>)
            .collect())
    }

    fn playlist_by_name(&self, name: &str) -> Option<Box<dyn Playlist>> {
        self.inner.LibrarySource().ok()?
            .Playlists().ok()?
            .ItemByName(name).ok()
            .and_then(|list| list.as_user_playlist())
            .map(|list| Box::new(list) as Box<dyn Playlist>)
    }

    fn playlist_by_id(&self, id: ItemId) -> Option<Box<dyn Playlist>> {
        itunes_get_playlist_by_id(&self.inner, id)
            .map(|list| Box::new(list) as Box<dyn Playlist>)
    }

    fn track_by_id(&self, id: ItemId) -> Option<Box<dyn Track>> {
        itunes_get_track_by_id(&self.inner, id)
            .map(|track| Box::new(track) as Box<dyn Track>)
    }
}

fn itunes_get_track_by_id(i_tunes: &iTunes, id: ItemId) -> Option<ITTrack> {
    i_tunes
        .LibraryPlaylist()
        .and_then(|lp| lp.Tracks())
        .ok()?
        .ItemByPersistentID(id.0)
        .ok()
}

fn itunes_get_playlist_by_id(i_tunes: &iTunes, id: ItemId) -> Option<ITUserPlaylist> {
    i_tunes.LibrarySource().ok()?
        .Playlists().ok()?
        .ItemByPersistentID(id.0).ok()
        .and_then(|list| list.as_user_playlist())
}




impl Playlist for ITUserPlaylist {
    fn name(&self) -> String {
        self.Name().unwrap_or_else(|err| format!("<error: {}>", err))
    }

    fn tracks(&self) -> Result<Vec<Box<dyn Track>>, Box<dyn Error>> {
        Ok(self.Tracks()?
            .iter()?
            .map(|t| Box::new(t) as Box<dyn Track>)
            .collect())
    }

    fn id(&self) -> ItemId {
        match self.persistent_id() {
            Err(err) => {
                // Should not happen, we're an ITPlaylist!
                log::warn!("Unable to get ID for playlist {}: {}", self.name(), err);
                ItemId(0)
            },
            Ok(id) => ItemId(id),
        }
    }

    fn change_contents_to(&self, new_content: &[ItemId]) -> Result<(), Box<dyn Error>> {
        // iTunes has no functions to reorder playlists, only add() and delete()
        // This will do.

        let playlist_id = ItemId(self.persistent_id()?);
        let iTunes = self.iTunes_instance();

        // Working around an iTunes bug (at least in 12.8.0.150)
        // For some reason, iTunes sometimes returns Err(ITUNES_E_OBJECTDELETED)
        // In this case, getting a new pointer to the playlist and try again should do
        let mut attempts = 0;
        loop {
            let list = match itunes_get_playlist_by_id(&iTunes, playlist_id) {
                Some(list) => list,
                None => return Err("Unable to get iTunes library from ID".into()),
            };

            match change_contents_to_inner(&list, new_content) {
                Ok(()) => break,
                // Err(ITUNES_E_OBJECTLOCKED) => // no need to work around (e.g. smart, read-only playlist)
                Err(err) => {
                    log::info!("Working around an iTunes bug ({}) when syncing the playlist. Trying again.", err);
                    attempts += 1;
                }
            }

            if attempts >= 10 {
                return Err(format!("Too many ({}) iTunes bugs when syncing this playlist. Giving up.", attempts).into());
            }
        }

        Ok(())
    }
}

fn change_contents_to_inner(playlist: &ITUserPlaylist, new_content: &[ItemId]) -> Result<(), Box<dyn Error>> {
    let get_i_th_track = |i| {
        // For some reason, using ItemByPlayOrder leads to strange bugs (looking like race conditions, such as getting many ITUNES_E_OBJECTDELETED errors)
        // Using the plain `item()` is fine.
        playlist.Tracks()?.item(i)
    };

    let mut i = 0;
    for required_id in new_content {
        i += 1;
        loop {
            match get_i_th_track(i) {
                Ok(i_th_track) => {
                    if i_th_track.persistent_id() == Ok(required_id.0) {
                        // Both lists match up to index i.
                        // Let's proceed to the next required track
                        log::trace!("Right track {:?} ({:?})", i_th_track.persistent_id(), i_th_track.name());
                        break;
                    } else {
                        // Let's remove the non-matching track. Maybe the next one will
                        log::trace!("Deleting {:?} ({:?})", i_th_track.persistent_id(), i_th_track.name());
                        i_th_track.Delete()?;
                    }
                },
                Err(err) if err.code() == windows::Win32::Media::Multimedia::NS_E_PROPERTY_NOT_FOUND => {
                    // The iTunes playlist has no more tracks. Let's add the one that is required.
                    let required_track = itunes_get_track_by_id(&playlist.iTunes_instance(), *required_id)
                        .ok_or(format!("Unable to find track with ID {:?}", required_id))?;

                    // playlist.AddTrack(&required_track.as_variant())?;
                    log::trace!("Adding {:?} ({:?})", required_id, required_track.name());
                    if let Err(err) = playlist.AddTrack(&required_track.as_variant()) {
                        println!("WTF? {:?} when adding {}", err, required_track.name());
                        return Err(Box::new(err) as Box<dyn Error>);
                    }
                    break;
                },
                Err(err) => {
                    // This may be an ITUNES_E_OBJECTDELETED, at least in iTunes 12.8.0.150. That's an iTunes bug.
                    return Err(Box::new(err) as Box<dyn Error>);
                }
            }
        }
    }

    // The head of the iTunes playlist matches the requirements.
    // Are there remaining tracks to remove?
    i += 1;
    while let Ok(extra_track) = get_i_th_track(i) {
        extra_track.Delete()?;
    }

    Ok(())
}

impl Track for ITTrack {
    fn name(&self) -> String {
        self.Name().unwrap_or_else(|err| format!("<error: {}>", err))
    }

    fn absolute_path(&self) -> Result<PathBuf, Box<dyn Error>> {
        let focdt = self.as_file_or_cd_track().ok_or_else(|| format!("Track {} is not a local file", self.name()))?;
        let location = focdt.Location()?;
        let mut path = PathBuf::from(location);
        if path.is_absolute() == false {
            let new_path = match path.canonicalize() {
                Err(err) => return Err(format!("Unable to get full path for song '{}': {}", self.name(), err).into()),
                Ok(canon) => canon,
            };
            path = new_path;
        }
        Ok(path)
    }

    fn id(&self) -> ItemId {
        match self.persistent_id() {
            Err(err) => {
                // Should not happen, we're an ITTrack!
                log::warn!("Unable to get ID for track {}: {}", self.name(), err);
                ItemId(0)
            },
            Ok(id) => ItemId(id),
        }
    }

    fn rating(&self, use_computed_ratings: bool) -> Option<u8> {
        match self.Rating().map(|r| r.stars()) {
            Err(err) => {
                // Should not happen, we're an ITTrack!
                log::warn!("Unable to get rating for track {}: {}", self.name(), err);
                None
            },
            Ok(None) => None,
            Ok(Some(stars)) => {
                if use_computed_ratings == false
                && self.as_file_or_cd_track().and_then(|foct| foct.ratingKind().ok()) == Some(ITRatingKind::ITRatingKindComputed)
                {
                    // Note: for some reason, many songs with no stars have a "computed rating".
                    log::debug!("Ignoring rating for track {}, because it is computed", self.name());
                    None
                } else {
                    Some(stars)
                }
            }
        }
    }

    fn set_rating(&self, new_rating: Option<u8>) -> Result<(), Box<dyn Error>> {
        Ok(self.set_Rating(itunes_com::wrappers::types::Rating::from_stars(new_rating))?)
    }

    fn file_size(&self) -> Result<usize, Box<dyn Error>> {
        let focdt = self.as_file_or_cd_track().ok_or_else(|| format!("Track {} is not a local file", self.name()))?;
        Ok(focdt.Size()?.try_into()?)
    }
}
