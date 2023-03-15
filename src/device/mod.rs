//! Devices are e.g. USB thumbdrives, MTP devices (or rather, their "functional objects"), etc.

use std::error::Error;
use std::path::Path;
use std::io::Read;

use crate::config::Config;
use crate::sync::SyncInfo;

pub mod disk;
pub mod mtp;
pub mod m3u;

pub const FOLDER_NAME: &str = "StarSync";
pub const MUSIC_FOLDER_NAME: &str = "music";
pub const CONFIG_FOLDER_NAME: &str = "config";
pub const CONFIG_FILE: &str = "starsync.json";
pub const SYNC_INFO_FILE: &str = "sync-info.json";

pub trait Device {
    // Required methods

    /// The device display name.
    ///
    /// It must be unique, and it must be possible to find this device from its name.
    fn name(&self) -> String;

    /// A handle to the `StarSync` folder on this device, if any.
    fn starsync_folder(&self) -> Option<Box<dyn Folder>>;
    fn config_folder(&self) -> Option<Box<dyn Folder>>;
    fn music_folder(&self) -> Option<Box<dyn Folder>>;
    fn create_folders(&self) -> Result<(), Box<dyn Error>>;
    fn remove_folders(&self) -> Result<(), Box<dyn Error>>;

    /// Write a file into the device, creating parent folders if needed
    fn push_music_file(&self, local_absolute_path: &Path, device_relative_path: &Path) -> Result<(), Box<dyn Error>>;
    /// Write a playlist into the device, creating parent folders if needed
    fn push_playlist(&self, content: &str, playlist_name: &Path) -> Result<(), Box<dyn Error>>;

    /// A hint to explain the user where to look for the config file
    fn config_display_path(&self) -> String;
    fn config(&self) -> Option<Config>;
    fn push_config(&self, config: &Config) -> Result<(), Box<dyn Error>>;
    fn previous_sync_infos(&self) -> Option<SyncInfo>;
    fn push_sync_infos(&self, sync_infos: &SyncInfo) -> Result<(), Box<dyn Error>>;


    // Provided methods

    /// Whether this device is inited already.
    ///
    /// This basically means "whether it has a StartSync folder"
    fn is_inited(&self) -> bool {
        self.starsync_folder().is_some()
    }

}

pub trait Folder {
    fn path(&self) -> &Path;
    // TODO: return an impl Iterator instead?
    fn sub_folders(&self) -> Result<Vec<Box<dyn Folder>>, Box<dyn Error>>;
    fn files(&self) -> Result<Vec<Box<dyn File>>, Box<dyn Error>>;
    fn file_at(&self, relative_path: &Path) -> Result<Box<dyn File>, Box<dyn Error>>;
}

pub trait File {
    fn path(&self) -> &Path;
    fn get_reader(&self) -> Result<Box<dyn Read>, Box<dyn Error>>;
    fn delete(&self) -> Result<(), Box<dyn Error>>;
}


pub fn list_devices(only_inited_devices: bool) -> Vec<Box<dyn Device>> {
    let mut devices = Vec::new();

    for mtp_dev in mtp::devices() {
        let dev = Box::new(mtp_dev) as Box<dyn Device>;
        if only_inited_devices == false || dev.is_inited() {
            devices.push(dev)
        }
    }

    for local_disk in disk::devices() {
        let dev = Box::new(local_disk) as Box<dyn Device>;
        if only_inited_devices == false || dev.is_inited() {
            devices.push(dev)
        }
    }

    devices
}

pub fn get(name: &str) -> Option<Box<dyn Device>> {
    // Not very smart, as it enumerates all devices.
    // But this is not too costly (compared to the rest of what StarSync does), so that's fine
    list_devices(false).into_iter().find(|dev| dev.name() == name)
}
