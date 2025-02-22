/// A RAII wrapper to tell the OS not to go to sleep while a sync is in progress
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
