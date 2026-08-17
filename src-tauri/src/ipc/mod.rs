//! Frontend IPC. Mirrors `src/api`.
//!
//! `config_*` · `status_get` · `guardian_*` · `wsl_list` · `disk_list` · `app_*`

pub mod app;
pub mod config;
pub mod guardian;
pub mod inventory;
pub mod status;

pub use app::*;
pub use config::*;
pub use guardian::*;
pub use inventory::*;
pub use status::*;
