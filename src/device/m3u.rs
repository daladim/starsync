use std::io::{Read, BufReader};
use std::path::{Path, PathBuf};
use std::iter::Iterator;

use m3u::Entry;

#[derive(Debug)]
pub struct M3u {
    content: Vec<PathBuf>,
}

impl M3u {
    pub fn parse(reader: Box<dyn Read>) -> Self {
        let buf_reader = BufReader::new(reader);
        let mut m3u_reader = m3u::EntryReader::new(buf_reader);
        Self{ content: m3u_reader
            .entries()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| match entry {
                Entry::Url(_) => None,
                Entry::Path(p) => Some(p)
            })
            .collect()
        }
    }

    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.content.iter().map(|buf| buf.as_path())
    }
}
