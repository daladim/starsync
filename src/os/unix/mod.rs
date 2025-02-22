use std::os::fd::RawFd;
use std::time::Duration;

use dbus::blocking::Connection;
use log::warn;

mod login1;
use login1::OrgFreedesktopLogin1Manager;

const TIMEOUT: Duration = Duration::from_secs(1);

/// A RAII wrapper to tell the OS not to go to sleep while a sync is in progress.
///
/// It uses systemd's d-bus API under the hood. See <https://systemd.io/INHIBITOR_LOCKS/>
pub struct PleaseStayAwake {
    /// The file descriptor of the lock. Should be `close`d when no longer needed. The OS will close it in case this program crashes.
    fd: Option<RawFd>,
}

impl PleaseStayAwake {
    pub fn new() -> Self {
        let fd = match Self::try_get_fd() {
            Ok(fd) => Some(fd),
            Err(err) => {
                warn!("Unable to inhibit system sleep: {err}");
                None
            }
        };
        Self{ fd }
    }

    pub fn try_get_fd() -> Result<RawFd, String> {
        let inhibitor_fd = Connection::new_system()
            .map_err(|err| format!("Unable to use DBus system bus: {err}"))?
            .with_proxy(
                "org.freedesktop.login1",
                "/org/freedesktop/login1",
                TIMEOUT,
            )
            .inhibit("sleep", "starsync", "sync in progress", "block")
            .map_err(|err| format!("Failed when calling the dbus 'inhibit' call: {err}"))?
            .into_fd();

        Ok(inhibitor_fd)
    }
}

impl Drop for PleaseStayAwake {
    fn drop(&mut self) {
        if let Some(fd) = self.fd {
            if let Err(err) = nix::unistd::close(fd) {
                warn!("Unable to release the sleep inhibition lock: {err}");
            }
        }
    }
}
