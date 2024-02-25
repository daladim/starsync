use sysinfo::{System, SystemExt};

pub fn current_hostname() -> String {
    let system = System::new();
    system.host_name().unwrap_or_else(|| "<unknown>".to_string())
}


/// A RAII wrapper to tell Windows not to go to sleep while a sync is in progress
pub struct PleaseStayAwake {}

impl PleaseStayAwake {
    pub fn new() -> Self {
        #[cfg(windows)]
        unsafe{
            windows::Win32::System::Power::SetThreadExecutionState(
                windows::Win32::System::Power::ES_CONTINUOUS |
                // windows::Win32::System::Power::ES_AWAYMODE_REQUIRED |
                windows::Win32::System::Power::ES_SYSTEM_REQUIRED
            );
        }

        #[cfg(not(windows))]
        log::warn!("Preventing the computer to go to sleep during a sync is only implemented on Windows");

        Self {}
    }
}

impl Drop for PleaseStayAwake {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe{
            windows::Win32::System::Power::SetThreadExecutionState(
                windows::Win32::System::Power::ES_CONTINUOUS
            );
        }
    }
}
