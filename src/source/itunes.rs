use std::error::Error;
use std::path::PathBuf;

use itunes_com::wrappers::iTunes;
use itunes_com::wrappers::UserPlaylist as ITUserPlaylist;
use itunes_com::wrappers::Track as ITTrack;
use itunes_com::wrappers::IITObjectWrapper;
use itunes_com::wrappers::IITPlaylistWrapper;
use itunes_com::wrappers::IITTrackWrapper;
use itunes_com::wrappers::ITunesRelatedObject;
use itunes_com::wrappers::Iterable;
use itunes_com::sys::ITSourceKind;

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
        let mut lists = Vec::new();

        for source in self.inner.Sources()?.iter()? {
            if source.Kind()? == ITSourceKind::ITSourceKindLibrary {
                for list in source
                    .Playlists()?
                    .iter()?
                    //
                    //
                    //
                    //
                    // TODO: what if we want to sync the 'Library' playlist?
                    //       macro to impl Playlist for both?
                    //
                    // .filter(|list| [ITPlaylistKind::ITPlaylistKindUser, ITPlaylistKind::ITPlaylistKindLibrary].contains(&list.Kind().unwrap_or(ITPlaylistKind::ITPlaylistKindUnknown)))
                    .filter_map(|list| list.as_user_playlist())
                {
                    lists.push(Box::new(list) as Box<dyn Playlist>);
                }
                break;
            }
        }

        Ok(lists)
    }

    fn playlist_by_name(&self, name: &str) -> Option<Box<dyn Playlist>> {
        self.inner.LibrarySource().ok()?
            .Playlists().ok()?
            .ItemByName(name).ok()
            .and_then(|list| list.as_user_playlist())
            .map(|list| Box::new(list) as Box<dyn Playlist>)
    }

    fn playlist_by_id(&self, id: ItemId) -> Option<Box<dyn Playlist>> {
        self.inner.LibrarySource().ok()?
            .Playlists().ok()?
            .ItemByPersistentID(id.0).ok()
            .and_then(|list| list.as_user_playlist())
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

        let get_i_th_track = |i| {
            // For some reason, using ItemByPlayCount leads to strange bugs (looking like race conditions, such as getting many ITUNES_E_OBJECTDELETED errors)
            // Using the plain `item()` is fine.
            self.Tracks()?.item(i)
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
                            break;
                        } else {
                            // Let's remove the non-matching track. Maybe the next one will
                            i_th_track.Delete()?;
                        }
                    },
                    Err(_) => {
                        // The iTunes playlist has no more tracks. Let's add the one that is required.
                        let required_track = itunes_get_track_by_id(&self.iTunes_instance(), *required_id)
                            .ok_or(format!("Unable to find track with ID {:?}", required_id))?;

                        // let foct = required_track.as_file_or_cd_track().ok_or_else(|| format!("Track {} is not a local file", required_track.name()))?;
                        // let location = foct.Location()?;
                        // self.AddFile(&location)?;

                        self.AddTrack(&required_track.as_variant())?;
                        break;
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

    fn rating(&self) -> Option<u8> {
        match self.Rating() {
            Err(err) => {
                // Should not happen, we're an ITTrack!
                log::warn!("Unable to get rating for track {}: {}", self.name(), err);
                None
            },
            Ok(rating) => rating.stars(),
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

// impl std::convert::From<ObjectIDs> for ItemId {
//     fn from(value: ObjectIDs) -> Self {
//         let mut bytes = [0; 16];
//         bytes[0..4].clone_from_slice(&value.sourceID.to_le_bytes());
//         bytes[4..8].clone_from_slice(&value.playlistID.to_le_bytes());
//         bytes[8..12].clone_from_slice(&value.trackID.to_le_bytes());
//         bytes[12..16].clone_from_slice(&value.databaseID.to_le_bytes());
//         ItemId(u128::from_le_bytes(bytes))
//     }
// }
