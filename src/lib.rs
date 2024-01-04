#![allow(clippy::bool_comparison)]  // because I like them

pub mod source;
pub mod device;
pub mod config;
pub mod sync;
pub mod utils;
mod common_path;

use crate::config::Config;

// BUG:
//  * lib restored from backup, fresh sync
//    daddy cool 3 -> 4
//    despacito 0 -> 1
//    daddy gonna pay crashed car 0 -> 4 then 4 -> 5
//    why so many unexpected star changes?
//
//    [2023-06-24T07:03:20Z INFO  starsync] ====ReverseSyncRatings=====
//    [2023-06-24T07:03:20Z DEBUG starsync] RetrievingDevicePlaylist("Favorites - 3 stars.m3u")
//    [2023-06-24T07:03:20Z DEBUG starsync] RetrievingDevicePlaylist("Favorites - 2 stars.m3u")
//    [2023-06-24T07:03:20Z DEBUG starsync] RetrievingDevicePlaylist("Favorites - 5 stars.m3u")
//    [2023-06-24T07:03:20Z DEBUG starsync] RetrievingDevicePlaylist("Favorites - 1 stars.m3u")
//    [2023-06-24T07:03:20Z DEBUG starsync] RetrievingDevicePlaylist("Favorites - 4 stars.m3u")
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "06 daddy's gonna pay for your crashe.mp3", new_rating: Some(5), current_rating_on_source: None }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "despacito ft. daddy yankee.mp3", new_rating: Some(1), current_rating_on_source: None }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "04 samedi soir sur la terre.mp3", new_rating: None, current_rating_on_source: Some(4) }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "13 etude de la stucture etrange.mp3", new_rating: None, current_rating_on_source: Some(4) }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "03 siberian khatru.mp3", new_rating: None, current_rating_on_source: Some(3) }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "07 you don't believe.mp3", new_rating: None, current_rating_on_source: Some(4) }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "04 a raft of penguins.mp3", new_rating: None, current_rating_on_source: Some(5) }
//    [2023-06-24T07:03:20Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "03 la cabane du pêcheur.mp3", new_rating: None, current_rating_on_source: Some(5) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "01 la corrida.mp3", new_rating: None, current_rating_on_source: Some(4) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "09 tambourine.mp3", new_rating: None, current_rating_on_source: Some(2) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "05 v. procession au crépuscule.mp3", new_rating: None, current_rating_on_source: Some(3) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "06 l'intranquillite.mp3", new_rating: None, current_rating_on_source: Some(3) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "02 body language (interpretation).mp3", new_rating: None, current_rating_on_source: Some(2) }
//    [2023-06-24T07:03:21Z DEBUG starsync] UpdatingSongRatingIntoSource { track_name: "daddy cool.mp3", new_rating: Some(4), current_rating_on_source: Some(3) }
//    [2023-06-24T07:03:21Z INFO  starsync] ====ListingFilesInSource=====
//
//
// TODO:
//  * disable windows going to sleep
//  * companion app to remove automatic ratings? (how is it done on applescript?)
//      or use https://samsoft.org.uk/iTunes/scripts.asp#ClearTrackAutoRating
//      see https://discussions.apple.com/thread/7392884?answerId=29522332022#29522332022
//  * issue with case (remove/re-copy songs that have different casing) (errors when removing from no_ratings hashset) (etc.)
//  * display progress (file sizes)
//  * replace all {:?} for paths with {} and path.display(), same for .to_string_lossy().to_string()
//  * upstream changes to diffy
//  * include version in previous info
//  * keep backup of previous info?
//  * keep logs on the last N syncs into the SD card
//  * playlists with radios: ignore radios, do not skip the whole m3u
//  * plylists to sync: not easy to add PL later on
//  * sync info: ID as hex
//  * doc about case-sensitiveness (how to compare)
//  * when I'm only syncing a few songs, that do not cover the five ratings, some rating PL are missing
//  * if "unable to get path of song, because of ERR_OBJ_DELETED" (happened at least once when a song was changed 5* -> 0*), we should not carry on the sync (otherwise, will delete songs on the device that should not be). Or, work around and retry to get the path instead
//
// TO TEST
//  * song in different PL
//  * same song added/removed/added to the same playlist. Does it still have the same ID (TrackDatabaseId?)
//  * is S2 case insensitive for m3u ?
//
// Have integration tests
//  * mocked iTunes should include Paul Ochon\song.mp3 and Paul OCHON\Other.mp3



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


/// A RAII wrapper to tell Windows not to go to sleep while a sync is in progress
pub struct PleaseStayAwake {}

impl PleaseStayAwake {
    pub fn new() -> Self {
        unsafe{
            windows::Win32::System::Power::SetThreadExecutionState(
                windows::Win32::System::Power::ES_CONTINUOUS |
                // windows::Win32::System::Power::ES_AWAYMODE_REQUIRED |
                windows::Win32::System::Power::ES_SYSTEM_REQUIRED
            );
        }

        Self {}
    }
}

impl Drop for PleaseStayAwake {
    fn drop(&mut self) {
        unsafe{
            windows::Win32::System::Power::SetThreadExecutionState(
                windows::Win32::System::Power::ES_CONTINUOUS
            );
        }
    }
}
