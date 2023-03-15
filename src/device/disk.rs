//! Local disks (especially removeable drives) can be suitable devices

use std::path::{Path, PathBuf};
use std::error::Error;
use std::io::Read;

use sysinfo::{System, SystemExt, RefreshKind, DiskExt};

use super::{File, Folder};
use crate::config::Config;
use crate::sync::SyncInfo;

#[cfg(feature = "debug_folder")]
use once_cell::sync::Lazy;
#[cfg(feature = "debug_folder")]
const DEBUG_FOLDER: Lazy<PathBuf> = Lazy::new(|| PathBuf::from(r"C:\Users\Public\Documents\"));

pub fn devices() -> Vec<LocalDevice> {
    let mut devs = Vec::new();

    let system = System::new_with_specifics(RefreshKind::new().with_disks_list());
    let disks = system.disks();
    for disk in disks {
        devs.push(LocalDevice{ mount_point: disk.mount_point().to_owned() });
    }

    #[cfg(feature = "debug_folder")]
    devs.push(LocalDevice{ mount_point: DEBUG_FOLDER.to_owned() });

    devs
}


pub struct LocalDevice {
    mount_point: PathBuf,
}

impl LocalDevice {
    fn starsync_folder_path(&self) -> PathBuf {
        self.mount_point.join(crate::device::FOLDER_NAME)
    }

    fn config_folder_path(&self) -> PathBuf {
        self.starsync_folder_path().join(crate::device::CONFIG_FOLDER_NAME)
    }

    fn music_folder_path(&self) -> PathBuf {
        self.starsync_folder_path().join(crate::device::MUSIC_FOLDER_NAME)
    }

    fn starsync_folder_impl(&self) -> Option<PathBuf> {
        let candidate = self.starsync_folder_path();
        if candidate.is_dir() {
            Some(candidate)
        } else {
            None
        }
    }

    fn config_folder_impl(&self) -> Option<PathBuf> {
        let candidate = self.config_folder_path();
        if candidate.is_dir() {
            Some(candidate)
        } else {
            None
        }
    }

    fn music_folder_impl(&self) -> Option<PathBuf> {
        let candidate = self.music_folder_path();
        if candidate.is_dir() {
            Some(candidate)
        } else {
            None
        }
    }
}

impl super::Device for LocalDevice {
    fn name(&self) -> String {
        format!("path://{}",
            self.mount_point.display().to_string().replace('\\', "/"),
        )
    }

    fn starsync_folder(&self) -> Option<Box<dyn Folder>> {
        self.starsync_folder_impl()
            .map(|folder| Box::new(LocalFolder(folder)) as Box<dyn Folder>)
    }

    fn config_folder(&self) -> Option<Box<dyn Folder>> {
        self.config_folder_impl()
            .map(|folder| Box::new(LocalFolder(folder)) as Box<dyn Folder>)
    }

    fn music_folder(&self) -> Option<Box<dyn Folder>> {
        self.music_folder_impl()
            .map(|folder| Box::new(LocalFolder(folder)) as Box<dyn Folder>)
    }

    fn create_folders(&self) -> Result<(), Box<dyn Error>> {
        let main_folder = self.starsync_folder_path();
        std::fs::create_dir(main_folder)?;

        let config_folder = self.config_folder_path();
        std::fs::create_dir(config_folder)?;

        let music_folder = self.music_folder_path();
        std::fs::create_dir(music_folder)?;

        Ok(())
    }

    fn remove_folders(&self) -> Result<(), Box<dyn Error>> {
        let target = self.starsync_folder_path();
        Ok(std::fs::remove_dir_all(target)?)
    }

    fn config_display_path(&self) -> String {
        self.config_folder_path().join(crate::device::CONFIG_FILE).display().to_string()
    }

    fn config(&self) -> Option<Config> {
        let config_folder = self.config_folder_impl()?;
        let config_file = config_folder.join(crate::device::CONFIG_FILE);
        let config_str = std::fs::File::open(config_file).ok()?;
        serde_json::from_reader(&config_str).ok()
    }

    fn push_config(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        let config_folder = self.config_folder_impl().ok_or("Missing StarSync folder")?;
        let config_path = config_folder.join(crate::device::CONFIG_FILE);
        let config_file = std::fs::File::create(config_path).map_err(|err| format!("Unable to write to device: {}", err))?;
        serde_json::to_writer_pretty(&config_file, config).map_err(|err| format!("Unable to write the configuration file: {}", err))?;
        Ok(())
    }

    fn previous_sync_infos(&self) -> Option<SyncInfo> {
        let config_folder = self.config_folder_impl()?;
        let info_path = config_folder.join(crate::device::SYNC_INFO_FILE);
        let sync_info_str = std::fs::File::open(info_path).ok()?;
        serde_json::from_reader(&sync_info_str).ok()
    }

    fn push_sync_infos(&self, sync_infos: &SyncInfo) -> Result<(), Box<dyn Error>> {
        let config_folder = self.config_folder_impl().ok_or("Missing StarSync folder")?;
        let info_path = config_folder.join(crate::device::SYNC_INFO_FILE);
        let info_file = std::fs::File::create(info_path).map_err(|err| format!("Unable to write to device: {}", err))?;
        serde_json::to_writer_pretty(&info_file, sync_infos).map_err(|err| format!("Unable to write the sync info file: {}", err))?;
        Ok(())
    }

    fn push_music_file(&self, local_absolute_path: &Path, device_relative_path: &Path) -> Result<(), Box<dyn Error>> {
        let dest_path = self.music_folder_path().join(device_relative_path);
        if let Some(dest_folder) = dest_path.parent() {
            if dest_folder.is_dir() == false {
                std::fs::create_dir_all(dest_folder)?;
            }
        }
        let mut dest = std::fs::File::create(&dest_path)?;
        let mut source = std::fs::File::open(local_absolute_path)?;
        Ok(std::io::copy(&mut source, &mut dest).map(|_| ())?)
    }

    fn push_playlist(&self, content: &str, playlist_name: &Path) -> Result<(), Box<dyn Error>> {
        let dest_path = self.starsync_folder_path().join(playlist_name);
        if let Some(dest_folder) = dest_path.parent() {
            if dest_folder.is_dir() == false {
                std::fs::create_dir_all(dest_folder)?;
            }
        }
        Ok(std::fs::write(dest_path, content)?)
    }
}

pub struct LocalFolder(PathBuf);
pub struct LocalFile(PathBuf);

impl LocalFolder {

}

impl Folder for LocalFolder {
    fn path(&self) -> &Path {
        &self.0
    }

    fn sub_folders(&self) -> Result<Vec<Box<dyn Folder>>, Box<dyn Error>> {
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&self.0)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                folders.push(Box::new(LocalFolder(entry.path())) as Box<dyn Folder>)
            }
        }
        Ok(folders)
    }

    fn files(&self) -> Result<Vec<Box<dyn File>>, Box<dyn Error>> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&self.0)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files.push(Box::new(LocalFile(entry.path())) as Box<dyn File>)
            }
        }
        Ok(files)
    }

    fn file_at(&self, relative_path: &Path) -> Result<Box<dyn File>, Box<dyn Error>> {
        let path = self.0.join(relative_path);
        if path.is_file() {
            Ok(Box::new(LocalFile(path)) as Box<dyn File>)
        } else {
            Err("Not found".into())
        }
    }
}

impl File for LocalFile {
    fn path(&self) -> &Path {
        &self.0
    }

    // fn get_content(&self) -> Result<String, Box<dyn Error>> {
    //     Ok(std::fs::read_to_string(&self.0)?)
    // }

    fn get_reader(&self) -> Result<Box<dyn Read>, Box<dyn Error>> {
        Ok(Box::new(
            std::fs::File::open(&self.0)?
        ) as Box<dyn Read>)
    }


    fn delete(&self) -> Result<(), Box<dyn Error>> {
        Ok(std::fs::remove_file(&self.0)?)
    }
}

