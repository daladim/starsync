use sysinfo::{System, SystemExt};

pub fn current_hostname() -> String {
    let system = System::new();
    system.host_name().unwrap_or_else(|| "<unknown>".to_string())
}

