#![allow(clippy::bool_comparison)]  // because I like them

pub mod source;
pub mod device;
pub mod config;
pub mod sync;
pub mod utils;
mod common_path;

use crate::config::Config;

//
//
//
//
// TODO:
//  * replace all {:?} for paths with {} and path.display()
//  * review ? or sender.send_warning (= distinction warning/error)
//  * upstream changes to diffy
//
// TO TEST
//  * song in different PL
//  * same song added/removed/added to the same playlist. Does it still have the same ID (TrackDatabaseId?)

#[derive(thiserror::Error, Debug)]
pub enum InitError {
    #[error("Source {0} not found")]
    SourceNotFound(String),
    #[error("Device {0} not found")]
    DeviceNotFound(String),
    #[error("This device looks inited already. To re-init it, first de-init, then try again")]
    AlreadyInited,
    #[error("Unable to write to the device")]
    WriteError,
}

#[derive(thiserror::Error, Debug)]
pub enum DeinitError {
    #[error("Device {0} not found")]
    DeviceNotFound(String),
    #[error("This device was not inited")]
    NotInited,
    #[error("Unable to write to the device")]
    WriteError,
}


/// Init a device, and return its config file
pub fn init_device(device_name: &str, source_name: &str) -> Result<String, InitError> {
    // Get a template config file
    let source = source::get(source_name).ok_or_else(|| InitError::SourceNotFound(source_name.to_string()))?;
    let all_playlists = source.playlists().map_err(|_| InitError::SourceNotFound(source_name.to_string()))?;
    let template_config = Config::new_template(source_name, &all_playlists);

    // Create the folder on the device
    let device = device::get(device_name).ok_or_else(|| InitError::DeviceNotFound(device_name.to_string()))?;
    if device.starsync_folder().is_some() {
        return Err(InitError::AlreadyInited);
    }
    device.create_folders().map_err(|_| InitError::WriteError)?;

    // Store the config into the device
    device.push_config(&template_config).map_err(|_| InitError::WriteError)?;

    Ok(device.config_display_path())
}

pub fn deinit_device(device_name: &str) -> Result<(), DeinitError> {
    let device = device::get(device_name).ok_or_else(|| DeinitError::DeviceNotFound(device_name.to_string()))?;
    if device.starsync_folder().is_none() {
        return Err(DeinitError::NotInited);
    }
    device.remove_folders().map_err(|_err| DeinitError::WriteError)?;
    Ok(())
}

