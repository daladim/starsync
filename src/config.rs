use serde::{Deserialize, Serialize};

use crate::source::Playlist;

pub fn val_true() -> bool{ true }
pub fn val_false() -> bool{ false }

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    source: String,
    #[serde(default = "crate::config::val_true")]
    include_ratings: bool,
    #[serde(default = "crate::config::val_false")]
    use_computed_ratings: bool,
    playlists: Vec<String>,
}

impl Config {
    pub fn new_template(source_name: &str, playlists: &[Box<dyn Playlist>]) -> Config {
        Config {
            source: source_name.to_string(),
            include_ratings: true,
            use_computed_ratings: false,
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

    pub fn include_ratings(&self) -> bool {
        self.include_ratings
    }

    pub fn use_computed_ratings(&self) -> bool {
        self.use_computed_ratings
    }
}
