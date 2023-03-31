//! Sources are e.g. iTunes

use std::error::Error;
use std::path::{Path, PathBuf};

pub mod itunes;
use itunes::ITunes;

//
//
// TODO: implement as hex
//
/// A song (or playlist) ID
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ItemId(pub u64);

/// The user rating of a track (None, or between 1 and 5 stars)
pub type Rating = Option<u8>;

pub trait Source {
    fn name(&self) -> &str;
    fn playlists(&self) -> Result<Vec<Box<dyn Playlist>>, Box<dyn Error>>;

    fn playlist_by_name(&self, name: &str) -> Option<Box<dyn Playlist>>;
    fn playlist_by_id(&self, id: ItemId) -> Option<Box<dyn Playlist>>;
    fn track_by_id(&self, id: ItemId) -> Option<Box<dyn Track>>;
}

pub trait Playlist {
    fn name(&self) -> String;
    fn tracks(&self) -> Result<Vec<Box<dyn Track>>, Box<dyn Error>>;
    fn id(&self) -> ItemId;
    /// Change the content of this playlist.
    ///
    /// This may merely re-order songs, but also remove or add songs.
    fn change_contents_to(&self, new_content: &[ItemId]) -> Result<(), Box<dyn Error>>;

    fn suitable_filename(&self) -> PathBuf {
        let mut sanitized_name = sanitize_filename::sanitize(self.name());
        sanitized_name.push_str(".m3u");
        sanitized_name.into()
    }

    fn to_m3u(&self, common_ancestor: &Path, prefix_to_add: &Path) -> Result<String, Box<dyn Error>> {
        let mut absolute_paths = Vec::new();
        for track in self.tracks()?.iter() {
            absolute_paths.push(track.absolute_path()?)
        }

        create_m3u(absolute_paths.iter(), common_ancestor, prefix_to_add)
    }
}

pub trait Track {
    fn name(&self) -> String;
    fn id(&self) -> ItemId;
    fn absolute_path(&self) -> Result<PathBuf, Box<dyn Error>>;
    fn rating(&self) -> Option<u8>;
    fn set_rating(&self, new_rating: Option<u8>) -> Result<(), Box<dyn Error>>;
    fn file_size(&self) -> Result<usize, Box<dyn Error>>;
}

pub fn create_m3u<T: Iterator<Item = P>, P: AsRef<Path>>(songs_absolute_paths: T, common_ancestor: &Path, prefix_to_add: &Path) -> Result<String, Box<dyn Error>> {
    let mut relative_paths = Vec::new();
    for path in songs_absolute_paths {
        let stripped_path = path.as_ref()
            .strip_prefix(common_ancestor)
            .unwrap_or(path.as_ref());
        let path_str = prefix_to_add.join(stripped_path).to_string_lossy().to_string().replace('\\', "/");
        relative_paths.push(path_str);
    }

    Ok(relative_paths.join("\r\n"))
}

pub fn list_sources() -> Vec<Box<dyn Source>> {
    let mut sources = Vec::new();

    // Is there an iTunes instance?
    if let Some(itunes) = ITunes::try_new() {
        sources.push(Box::new(itunes) as Box<dyn Source>);
    }

    // TODO: could we do anything with shared iTunes libraries on the network?

    sources
}

pub fn get(name: &str) -> Option<Box<dyn Source>> {
    // Not very smart, as it enumerates all sources.
    // For now, we only have one source, so that's fine
    list_sources().into_iter().find(|source| source.name() == name)
}