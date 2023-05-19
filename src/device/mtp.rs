//! MTP devices

//
//
//
//
// TODO: revert
#![allow(unused_variables)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::io::Read;

use winmtp::{Provider, error::ItemByPathError};
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
pub struct FolderObject(Object, PathBuf);
pub struct FileObject(Object, PathBuf);

impl RootObject {
    fn starsync_folder_impl(&self) -> Result<(Object, PathBuf), ItemByPathError> {
        let path = crate::device::FOLDER_NAME;
        self.1
            .object_by_path(&Path::new(path))
            .map(|o| (o, PathBuf::from(path)))
    }

    fn config_folder_impl(&self) -> Result<(Object, PathBuf), ItemByPathError> {
        let path = Path::new(crate::device::FOLDER_NAME).join(crate::device::CONFIG_FOLDER_NAME);
        self.1
            .object_by_path(&path)
            .map(|o| (o, path))
    }

    fn music_folder_impl(&self) -> Result<(Object, PathBuf), ItemByPathError> {
        let path = Path::new(crate::device::FOLDER_NAME).join(crate::device::MUSIC_FOLDER_NAME);
        self.1
            .object_by_path(&path)
            .map(|o| (o, path))
    }
}

impl super::Device for RootObject {
    fn name(&self) -> String {
        format!("mtp://{}/{}",
            self.0.friendly_name(),
            self.1.name().to_string_lossy(),
        )
    }

    fn starsync_folder(&self) -> Option<Box<dyn Folder>> {
        self.starsync_folder_impl()
            .map(|obj| Box::new(FolderObject(obj.0, obj.1)) as Box<dyn Folder>)
            .ok()
    }

    fn config_folder(&self) -> Option<Box<dyn Folder>> {
        self.config_folder_impl()
            .map(|obj| Box::new(FolderObject(obj.0, obj.1)) as Box<dyn Folder>)
            .ok()
    }

    fn music_folder(&self) -> Option<Box<dyn Folder>> {
        self.music_folder_impl()
            .map(|obj| Box::new(FolderObject(obj.0, obj.1)) as Box<dyn Folder>)
            .ok()
    }

    fn create_folders(&self) -> Result<(), Box<dyn Error>> {
        // Does the root folder even exist?
        if self.1.sub_folders()?.any(|folder| &folder.name().to_os_string() == &OsStr::new(crate::device::FOLDER_NAME)) == false {
            self.1
                .create_subfolder(&OsStr::new(crate::device::FOLDER_NAME))?;
        }
        let starsync_folder = self.1.object_by_path(&Path::new(crate::device::FOLDER_NAME))?;

        let mut has_music = false;
        let mut has_config = false;

        for existing_sub in starsync_folder.sub_folders()? {
            let existing_sub_name = existing_sub.name().to_string_lossy().to_string();
            if &existing_sub_name == crate::device::MUSIC_FOLDER_NAME {
                has_music = true;
            }
            if &existing_sub_name == crate::device::CONFIG_FOLDER_NAME {
                has_config = true;
            }
        }

        if has_music == false {
            starsync_folder.create_subfolder(&OsStr::new(crate::device::MUSIC_FOLDER_NAME))?;
        }
        if has_config == false {
            starsync_folder.create_subfolder(&OsStr::new(crate::device::CONFIG_FOLDER_NAME))?;
        }

        Ok(())
    }

    fn remove_folders(&self) -> Result<(), Box<dyn Error>> {
        self.starsync_folder_impl()?
            .0.delete(true)?;
        Ok(())
    }

    fn config_display_path(&self) -> String {
        format!("{}/{}",
            self.name(),
            crate::device::CONFIG_FILE
        )
    }

    fn config(&self) -> Option<Config> {
        let config_file = self
            .config_folder_impl()
            .ok()?
            .0
            .object_by_path(&Path::new(crate::device::CONFIG_FILE))
            .ok()?
            .open_read_stream()
            .ok()?;
        serde_json::from_reader(config_file).ok()
    }

    fn push_config(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        let config_json = serde_json::to_string_pretty(&config).map_err(|err| format!("Unable to serialize the configuration: {}", err))?;

        Ok(self.config_folder_impl()?
            .0
            .push_data(&OsStr::new(crate::device::CONFIG_FILE), config_json.as_bytes(), true)?)
    }

    fn previous_sync_infos(&self) -> Option<SyncInfo> {
        let reader = self.config_folder_impl()
            .ok()?
            .0
            .object_by_path(&Path::new(crate::device::SYNC_INFO_FILE))
            .ok()?
            .open_read_stream()
            .ok()?;

        serde_json::from_reader(reader).ok()
    }

    fn push_sync_infos(&self, sync_infos: &SyncInfo) -> Result<(), Box<dyn Error>> {
        let info_json = serde_json::to_string_pretty(&sync_infos).map_err(|err| format!("Unable to serialize the sync info: {}", err))?;

        Ok(self.config_folder_impl()?
            .0
            .push_data(&OsStr::new(crate::device::SYNC_INFO_FILE), info_json.as_bytes(), true)?)
    }

    fn push_music_file(&self, local_absolute_path: &Path, device_relative_path: &Path) -> Result<(), Box<dyn Error>> {
        let device_folder_path = device_relative_path.parent().ok_or(format!("Path has no parent folder"))?;

        // Create the parent dir, if needed
        self
            .music_folder_impl()?
            .0
            .create_subfolder_recursive(device_folder_path)?;

        let device_folder = self.music_folder_impl()?.0.object_by_path(device_folder_path)?;
        device_folder.push_file(local_absolute_path, true)?;

        Ok(())
    }

    fn push_playlist(&self, content: &str, playlist_name: &OsStr) -> Result<(), Box<dyn Error>> {
        Ok(self.starsync_folder_impl()?
            .0
            .push_data(playlist_name, content.as_bytes(), true)?)
    }
}

impl Folder for FolderObject {
    fn path(&self) -> &Path {
        &self.1
    }

    fn sub_folders(&self) -> Result<Vec<Box<dyn Folder>>, Box<dyn Error>> {
        Ok(self
            .0
            .sub_folders()?
            .map(|obj| {
                let sub_path = obj.name().to_os_string();
                let path = self.1.join(sub_path);
                Box::new(FolderObject(obj, path)) as Box<dyn Folder>
            })
            .collect())
    }

    fn files(&self) -> Result<Vec<Box<dyn File>>, Box<dyn Error>> {
        Ok(self
            .0
            .children()?
            .filter(|obj| obj.object_type().is_file_like())
            .map(|obj| {
                let sub_path = obj.name().to_os_string();
                let path = self.1.join(sub_path);
                Box::new(FileObject(obj, path)) as Box<dyn File>
            })
            .collect())
    }

    fn file_at(&self, relative_path: &Path) -> Result<Box<dyn File>, Box<dyn Error>> {
        Ok(self
            .0
            .object_by_path(relative_path)
            .map(|obj| Box::new(FileObject(obj, PathBuf::from(relative_path))) as Box<dyn File>)?)
    }
}

impl File for FileObject {
    fn path(&self) -> &Path {
        &self.1
    }

    fn get_reader(&self) -> Result<Box<dyn Read>, Box<dyn Error>> {
        let reader = self.0.open_read_stream()?;
        Ok(Box::new(reader) as Box<dyn Read>)
    }

    fn delete(&mut self) -> Result<(), Box<dyn Error>> {
        self.0.delete(false)?;
        Ok(())
    }
}

