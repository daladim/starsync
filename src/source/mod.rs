//! Sources are e.g. iTunes, Rhythmbox, etc.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::num::NonZeroU8;

#[cfg(windows)]
pub mod itunes;

#[cfg(unix)]
pub mod rhythmbox;

mod serde_u64_hex_utils;

/// A song ID
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TrackId(
    #[serde(with = "serde_u64_hex_utils")]
    pub u64
);

/// A playlist ID
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlaylistId{
    /// Persistent ID is a number
    /// (e.g. iTunes)
    #[serde(with = "serde_u64_hex_utils")]
    Number(u64),
    /// Persistent ID is a string
    /// (e.g. Rhythmbox).
    /// Note that we may "lose track" of the playlist if it gets renamed...
    Name(String),
}


/// The user rating of a track (None, or between 1 and 5 stars)
pub type Rating = Option<NonZeroU8>;

pub trait Source {
    fn name(&self) -> &str;
    fn playlists(&self) -> Result<Vec<Box<dyn Playlist>>, Box<dyn Error>>;

    fn playlist_by_name(&self, name: &str) -> Option<Box<dyn Playlist>>;
    fn playlist_by_id(&self, id: &PlaylistId) -> Option<Box<dyn Playlist>>;
    fn track_by_id(&self, id: TrackId) -> Option<Box<dyn Track>>;
}

pub trait Playlist {
    fn name(&self) -> String;
    fn tracks(&self) -> Result<Vec<Box<dyn Track>>, Box<dyn Error>>;
    fn id(&self) -> PlaylistId;
    /// Change the content of this playlist.
    ///
    /// This may merely re-order songs, but also remove or add songs.
    fn change_contents_to(&self, new_content: &[TrackId]) -> Result<(), Box<dyn Error>>;

    fn suitable_filename(&self) -> String {
        let mut sanitized_name = sanitize_filename::sanitize(self.name());
        sanitized_name.push_str(".m3u");
        sanitized_name
    }

    fn to_m3u(&self, common_ancestor: &Path, prefix_to_add: &Path) -> Result<String, Box<dyn Error>> {
        let mut relative_paths = Vec::new();
        for track in self.tracks()?.iter() {
            let relative_path = track.absolute_path()?
                .strip_prefix(common_ancestor)
                .map(|r| r.to_path_buf())
                .map_err(|_| format!("Track '{}' is not a child of the common ancestor '{}'", track.name(), common_ancestor.display()))?;
            relative_paths.push(relative_path)
        }

        create_m3u(relative_paths.iter(), prefix_to_add)
    }
}

pub trait Track {
    fn name(&self) -> String;
    fn id(&self) -> TrackId;
    fn absolute_path(&self) -> Result<PathBuf, Box<dyn Error>>;
    fn rating(&self, use_computed_ratings: bool) -> Rating;
    fn set_rating(&self, new_rating: Rating) -> Result<(), Box<dyn Error>>;
    fn file_size(&self) -> Result<usize, Box<dyn Error>>;
}

pub fn create_m3u<T: Iterator<Item = P>, P: AsRef<Path>>(songs_relative_paths: T, prefix_to_add: &Path) -> Result<String, Box<dyn Error>> {
    let mut relative_paths = Vec::new();
    for relative_path in songs_relative_paths {
        let path_str = prefix_to_add.join(relative_path).to_string_lossy().to_string().replace('\\', "/");
        relative_paths.push(path_str);
    }

    Ok(relative_paths.join("\r\n"))
}

pub fn list_sources() -> Vec<Box<dyn Source>> {
    let mut sources = Vec::new();

    // Is there an iTunes instance?
    #[cfg(windows)]
    if let Some(itunes) = itunes_win::ITunes::try_new() {
        sources.push(Box::new(itunes) as Box<dyn Source>);
    }

    #[cfg(unix)]
    if let Some(rhythmbox) = rhythmbox::Rhythmbox::try_new() {
        sources.push(Box::new(rhythmbox) as Box<dyn Source>);
    }

    // TODO: could we do anything with shared iTunes libraries on the network?

    sources
}

pub fn get(name: &str) -> Option<Box<dyn Source>> {
    // Not very smart, as it enumerates all sources.
    // For now, we only have one source, so that's fine
    list_sources().into_iter().find(|source| source.name() == name)
}
