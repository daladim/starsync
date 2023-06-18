use std::sync::atomic::AtomicUsize;

use super::{ItemId, Rating};

//
//
//
//
//
// TODO: send progress (file size/total)
//

pub struct Sender {
    tx: std::sync::mpsc::Sender<Message>,
    n_warns: AtomicUsize,
}

pub type Receiver = std::sync::mpsc::Receiver<Message>;

pub fn channel() -> (Sender, Receiver) {
    let (tx, rx) = std::sync::mpsc::channel();
    (Sender{ tx, n_warns: AtomicUsize::new(0) }, rx)
}

impl Sender {
    pub fn send(&self, message: Message) {
        if let &Message::Warning(_) = &message {
            self.n_warns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        // An error means the receiving end is deconnected.
        // There's nothing much we can do to recover. We might as well ignore the error.
        let _ignored_error = self.tx.send(message);
    }

    /// Convenience function
    pub fn send_warning<T: ToString>(&self, text: T) {
        self.send(Message::Warning(text.to_string()))
    }

    /// Convenience function
    pub fn send_info<T: ToString>(&self, text: T) {
        self.send(Message::Info(text.to_string()))
    }

    /// Convenience function
    pub fn send_progress(&self, progress: Progress) {
        self.send(Message::Progress(progress))
    }

    pub fn warnings_count(&self) -> usize {
        self.n_warns.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub enum Message {
    /// Info about the current step we have reached
    Progress(Progress),
    /// Reading a playlist from the device
    RetrievingDevicePlaylist(String),
    /// Reverse syncing a playlist
    ReverseSyncPlaylist(String),
    /// Reverse-updating a playlist into the source
    UpdatingPlaylistIntoSource{ new_content: Vec<ItemId>},
    /// Importing a rating change back into the source
    UpdatingSongRatingIntoSource{ track_name: String, new_rating: Rating },
    /// A music file is about to be copied
    PushingFile(String),
    /// A music file is about to be removed
    RemovingFile(String),
    /// A playlist file is about to be copied
    PushingPlaylist(String),
    /// A playlist file is about to be removed
    RemovingPlaylist(String),
    /// An arbitrary info
    Info(String),
    /// A non-fatal warning
    Warning(String),
}

#[derive(Debug)]
pub enum Progress {
    /// Sync has started
    Started,
    /// Scanning the files on the device
    ListingFilesOnDevice,
    /// Reverse-syncing playlists
    ReverseSyncPlaylists,
    /// Reverse-syncing song ratings
    ReverseSyncRatings,
    /// Generating the list of files to sync
    ListingFilesInSource,
    /// Currently syncing files
    SyncingFiles,
    /// Pushing the updated playlists to the device
    PushingPlaylists,
    /// Pushing song ratings to the device
    PushingRatings,
    /// Update the status of the latest sync onto the device, so that it knows how it went
    UpdatingSyncInfo,
    /// Sync has completed
    Done,
}
