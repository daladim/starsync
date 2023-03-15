//! MTP devices

//
//
//
//
// TODO: revert
#![allow(unused_variables)]

use std::path::Path;
use std::error::Error;
use std::io::Read;

use winmtp::Provider;
use winmtp::device::BasicDevice;
use winmtp::device::device_values::AppIdentifiers;
use winmtp::object::Object;

use crate::config::Config;
use crate::sync::SyncInfo;
use super::{File, Folder};





pub fn devices() -> Vec<RootObject> {
    let mut devs = Vec::new();

    if let Ok(mtp_provider) = Provider::new() {
        let app_id = winmtp::make_current_app_identifiers!();

        if let Ok(mtp_devices) = mtp_provider.enumerate_devices() {
            // A device can have multiple functional devices
            for mtp_dev in &mtp_devices {
                if let Ok(functional_objects) = mtp_dev.open(&app_id)
                    .and_then(|dev| dev.content())
                    .and_then(|content| content.functional_objects()) {
                        for functional_obj in functional_objects {
                            devs.push(RootObject(BasicDevice::clone(mtp_dev), functional_obj))
                        }
                    }
            }
        }
    }

    devs
}

pub struct RootObject(BasicDevice, Object);
pub struct FolderObject(Object);
pub struct FileObject(Object);

impl super::Device for RootObject {
    fn name(&self) -> String {
        format!("mtp://{}/{}",
            self.0.friendly_name(),
            self.1.name().to_string_lossy(),
        )
    }

    fn starsync_folder(&self) -> Option<Box<dyn Folder>> {
        self.1
            .object_by_path(Path::new("StarSync"))
            .map(|obj| Box::new(FolderObject(obj)) as Box<dyn Folder>)
            .ok()
    }

    fn config_folder(&self) -> Option<Box<dyn Folder>> {
        todo!();
    }

    fn music_folder(&self) -> Option<Box<dyn Folder>> {
        todo!();
    }

    fn create_folders(&self) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    fn remove_folders(&self) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    fn config_display_path(&self) -> String {
        todo!();
    }

    fn config(&self) -> Option<Config> {
        todo!();
    }

    fn push_config(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    fn previous_sync_infos(&self) -> Option<SyncInfo> {
        todo!();
    }

    fn push_sync_infos(&self, sync_infos: &SyncInfo) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    fn push_music_file(&self, local_absolute_path: &Path, device_relative_path: &Path) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    fn push_playlist(&self, content: &str, playlist_name: &Path) -> Result<(), Box<dyn Error>> {
        todo!();
    }
}

impl Folder for FolderObject {
    fn path(&self) -> &Path {
        todo!();
    }

    fn sub_folders(&self) -> Result<Vec<Box<dyn Folder>>, Box<dyn Error>> {
        todo!();
    }

    fn files(&self) -> Result<Vec<Box<dyn File>>, Box<dyn Error>> {
        todo!();
    }

    fn file_at(&self, relative_path: &Path) -> Result<Box<dyn File>, Box<dyn Error>> {
        todo!();
    }
}

impl File for FileObject {
    fn path(&self) -> &Path {
        todo!();
    }

    fn get_reader(&self) -> Result<Box<dyn Read>, Box<dyn Error>> {
        todo!();
    }

    fn delete(&self) -> Result<(), Box<dyn Error>> {
        todo!();
    }
}

