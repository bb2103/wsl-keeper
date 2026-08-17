//! WSL CLI, physical disks, and the elevated mount task.

pub mod disk;
pub mod mount;
pub mod wsl;

use std::path::PathBuf;

pub fn system32_dir() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let sysnative = PathBuf::from(&root).join("Sysnative");
    if sysnative.is_dir() {
        sysnative
    } else {
        PathBuf::from(root).join("System32")
    }
}
