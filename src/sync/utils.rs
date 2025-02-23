use std::collections::{HashMap, HashSet};
use std::path::{PathBuf, Path};
use std::num::NonZeroU8;

use crate::source::{TrackId, Rating};

const RATINGS_PLAYLIST_PREFIX: &str = "Favourites - ";
const RATINGS_PLAYLIST_SUFFIX: &str = " stars.m3u";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestedPlaylistKind {
    /// Regular user playlists
    Regular,
    /// Synthetic playlists that actually is just a bag of songs that have a given rating
    Ratings,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ActualPlaylistKind {
    /// A regular user playlist
    Regular(String),
    /// A synthetic playlist that actually is just a bag of songs that have a given rating
    Ratings(NonZeroU8),
}

impl ActualPlaylistKind {
    pub fn classify(m3u_file_name: &str) -> Self {
        match m3u_file_name
            .strip_prefix(RATINGS_PLAYLIST_PREFIX)
            .and_then(|rest| rest.strip_suffix(RATINGS_PLAYLIST_SUFFIX))
            .and_then(|digit| digit.parse::<NonZeroU8>().ok())
            .filter(|stars_nzu| *stars_nzu <= NonZeroU8::new(5).unwrap())
        {
            Some(stars) => Self::Ratings(stars),
            None => Self::Regular(m3u_file_name.to_string()),
        }
    }

    #[allow(dead_code)]  // May be used one day
    fn matches(&self, requested: RequestedPlaylistKind) -> bool {
        matches!( (self, requested),
            (Self::Regular(_), RequestedPlaylistKind::Regular) |
            (Self::Ratings(_), RequestedPlaylistKind::Ratings)
        )
    }

    pub fn stars(&self) -> Rating {
        match self {
            Self::Ratings(stars) => Some(*stars),
            _ => None,
        }
    }
}

pub fn favorites_playlist_name(rating: u8) -> String {
    format!("{}{}{}", RATINGS_PLAYLIST_PREFIX, rating, RATINGS_PLAYLIST_SUFFIX)
}


#[derive(Debug)]
pub struct FileData {
    /// Size (in bytes) of the file
    pub file_size: usize,
    pub id: TrackId,
    pub rating: Rating,
}

#[derive(Debug)]
pub struct FileSet {
    pub common_ancestor: PathBuf,
    /// A hashmap indexed by relative paths on the source
    pub files_data: HashMap<PathBuf, FileData>,
    /// Total size of this file set, in bytes
    pub total_size: usize,
}

impl FileSet {
    pub fn song_paths_by_rating(&self) -> HashMap<u8, Vec<&Path>> {
        let mut rated_songs = HashMap::new();
        rated_songs.insert(1, Vec::new());
        rated_songs.insert(2, Vec::new());
        rated_songs.insert(3, Vec::new());
        rated_songs.insert(4, Vec::new());
        rated_songs.insert(5, Vec::new());

        for (path, data) in &self.files_data {
            data.rating
                .and_then(|stars| rated_songs.get_mut(&stars.get()))
                .map(|this_rating| this_rating.push(path.as_path()));
        }

        rated_songs
    }
}


pub struct CaseInsensitiveDiff<'a> {
    // iterator of the first set
    iter: std::collections::hash_set::Iter<'a, PathBuf>,
    // the second set
    other: HashSet<PathBuf>,
}


pub fn case_insensitive_difference<'a>(left: &'a HashSet<PathBuf>, right: &HashSet<PathBuf>) -> CaseInsensitiveDiff<'a> {
    CaseInsensitiveDiff{
        iter: left.iter(),
        other: right.iter().map(|item| PathBuf::from(item.to_string_lossy().to_lowercase())).collect(),
    }
}

impl<'a> std::iter::Iterator for CaseInsensitiveDiff<'a> {
    type Item = &'a PathBuf;

    #[inline]
    fn next(&mut self) -> Option<&'a PathBuf> {
        loop {
            let elt = self.iter.next()?;
            if !self.other.contains(Path::new(&elt.to_string_lossy().to_lowercase())) {
                return Some(elt);
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_classify_playlist() {
        assert_eq!(ActualPlaylistKind::classify("Favourites - 1 stars.m3u"), ActualPlaylistKind::Ratings(1));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 2 stars.m3u"), ActualPlaylistKind::Ratings(2));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 3 stars.m3u"), ActualPlaylistKind::Ratings(3));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 4 stars.m3u"), ActualPlaylistKind::Ratings(4));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 5 stars.m3u"), ActualPlaylistKind::Ratings(5));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 0 stars.m3u"), ActualPlaylistKind::Regular("Favourites - 0 stars.m3u".to_string()));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 6 stars.m3u"), ActualPlaylistKind::Regular("Favourites - 6 stars.m3u".to_string()));
        assert_eq!(ActualPlaylistKind::classify("Favourites - 1 stars"), ActualPlaylistKind::Regular("Favourites - 1 stars".to_string()));
        assert_eq!(ActualPlaylistKind::classify("Favorites - 1 stars.m3u"), ActualPlaylistKind::Regular("Favorites - 1 stars.m3u".to_string()));
        assert_eq!(ActualPlaylistKind::classify("abc.m3u"), ActualPlaylistKind::Regular("abc.m3u".to_string()));

        assert_eq!(ActualPlaylistKind::classify(&favorites_playlist_name(3)), ActualPlaylistKind::Ratings(3));

        assert!(ActualPlaylistKind::classify("abc.m3u").matches(RequestedPlaylistKind::Regular));
        assert!(ActualPlaylistKind::classify("Favourites - 4 stars.m3u").matches(RequestedPlaylistKind::Ratings));
    }

    #[test]
    fn test_case_insensitive_sets() {
        let left: HashSet<PathBuf> = [
            "C:\\Users\\John\\File.DAT",
            "C:\\Users\\jack\\stuff.mp3",
            "left",
            "C:\\Users\\paul\\fancy.exe",
        ].iter().map(|item| PathBuf::from(item)).collect();

        let right: HashSet<PathBuf> = [
            "right",
            "c:\\users\\john\\file.dat",
            "C:\\Users\\jack\\stuff.mp3",
            "C:\\Users\\PAUL\\Fancy.exe",
        ].iter().map(|item| PathBuf::from(item)).collect();

        let diff: Vec<PathBuf> = case_insensitive_difference(&left, &right).map(|p| p.clone()).collect();
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.get(0), Some(&PathBuf::from("left")));
    }
}
