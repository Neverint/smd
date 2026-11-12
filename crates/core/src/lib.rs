//! Pure logic shared by the Scrub My Disks backend.
//!
//! Nothing in this crate touches the file system, the registry or any Windows API,
//! so every function here can be unit-tested on any machine.

pub mod apps;
pub mod cleanup;
pub mod cmdline;
pub mod format;
pub mod hardware;
pub mod health;
pub mod safety;
pub mod space;
pub mod startup;
