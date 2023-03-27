//! Information about a sync session

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::source::ItemId;
use crate::source::Rating;
use super::PlaylistsSet;

/// Some info about a sync
///
/// Usually, they will be retrieved from the device.
/// Which means they have the values **as they were during the previous sync**.
#[derive(Serialize, Deserialize)]
pub struct SyncInfo {
    /// The hostname of the computer the sync is performed on
    hostname: String,
    /// The timestamp of this sync
    timestamp: time::OffsetDateTime,
    common_ancestor: PathBuf,
    song_data: HashMap<PathBuf, (ItemId, Rating)>,
    playlists: PlaylistsSet,
}

impl SyncInfo {
    pub fn new(common_ancestor: PathBuf, song_data: HashMap<PathBuf, (ItemId, Rating)>, playlists: PlaylistsSet) -> Self {
        let hostname = crate::utils::current_hostname();
        let timestamp = OffsetDateTime::now_utc();
        Self{ hostname, timestamp, common_ancestor, song_data, playlists }
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn timestamp(&self) -> &time::OffsetDateTime {
        &self.timestamp
    }

    pub fn id_for_relative_path(&self, relative_path: &Path) -> Option<ItemId> {
        let full_path = self.common_ancestor.join(relative_path);
        self.song_data.get(&full_path).map(|data| data.0)
    }

    pub fn id_for_full_path(&self, path: &Path) -> Option<ItemId> {
        self.song_data.get(path).map(|data| data.0)
    }

    pub fn rating_for_id(&self, needle: ItemId) -> Rating {
        self.song_data
            .iter()
            .find(|(_, (id, _))| *id == needle)
            .and_then(|(_, (_, rating))| *rating)
    }

    pub fn path_for_id(&self, id: ItemId) -> Option<PathBuf> {
        self.song_data.iter()
            .find(|(_, (stored_id, _))| *stored_id == id)
            .map(|(path, _)| path.clone())
    }

    pub fn playlist(&self, name: &str) -> Option<&(ItemId, Vec<ItemId>)> {
        self.playlists.get(name)
    }

    pub fn has_playlist_file_name<S: AsRef<str>>(&self, needle: S) -> bool {
        self.playlists.iter().any(|(file_name, _)| file_name == needle.as_ref())
    }
}
