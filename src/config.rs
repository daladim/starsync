use serde::{Deserialize, Serialize};

use crate::source::Playlist;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    source: String,
    include_stars: bool,
    playlists: Vec<String>,
}

impl Config {
    pub fn new_template(source_name: &str, playlists: &[Box<dyn Playlist>]) -> Config {
        Config {
            source: source_name.to_string(),
            include_stars: true,
            playlists: playlists.iter().map(|p| p.name()).collect()
        }
    }

    pub fn new(config_str: &str) -> Result<Config, serde_json::Error> {
        serde_json::from_str(config_str)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn playlists(&self) -> &[String] {
        &self.playlists
    }

    pub fn include_stars(&self) -> bool {
        self.include_stars
    }
}
