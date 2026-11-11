//! File system and Windows API access for Scrub My Disks.
//!
//! Everything here is read-only except `cleanup::remove_items`, `cleanup::restore`
//! and `system::empty_recycle_bin`, which the app only calls after the user reviews
//! the results and clicks the action button.

pub mod cleanup;
pub mod disks;
pub mod powershell;
pub mod system;
pub mod walk;
