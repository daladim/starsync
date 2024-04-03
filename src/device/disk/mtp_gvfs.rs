//! GNOME exposes MTP devices as mount points in the filesystem.
//!
//! This may not be the most efficient, but this definitely is convenient.

use std::error::Error;
use std::path::PathBuf;
use std::process::Command;


pub fn devices() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    // What is my user ID?
    let id_output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|err| format!("Unable to get the current user ID: {err:?}"))?;

    let my_id: u32 = if id_output.status.success() == false {
        return Err("Unable to get the current user ID".into());
    } else {
        let s_id = String::from_utf8(id_output.stdout)?;
        let stripped = s_id.strip_suffix("\n").unwrap_or(&s_id);
        stripped.parse()?
    };

    // Are there currently munted MTP devices?
    let mnt = PathBuf::from(format!("/run/user/{my_id}/gvfs/"));
    let mtp_roots = std::fs::read_dir(&mnt)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().ok().map(|ty| ty.is_dir()).unwrap_or(false))
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("mtp:"))
        .map(|entry| entry.path());

    // Let's include as well the top-level children of those roots, as Android devices often expose the actual disks (e.g. internal storage and SD card) as children of this root
    // TODO: find a way to know whether we can write in this root. If not, we probably need to walk the children.
    let mut mtp_paths = Vec::new();
    for root in mtp_roots {
        mtp_paths.push(root.clone());
        for child in std::fs::read_dir(root)? {
            mtp_paths.push(child?.path());
        }
    }

    Ok(mtp_paths)
}
