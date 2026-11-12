//! File system and Windows API access for Scrub My Disks.
//!
//! Everything here is read-only except the functions that change something on purpose:
//! `cleanup::remove_items`, `cleanup::restore`, `system::empty_recycle_bin`,
//! `startup::set_enabled`, `apps::uninstall`, `apps::backup_and_delete_key`,
//! `apps::restore_key`, `winget::install_module` and `winget::update`. The app calls them
//! only after the user reviews the results and clicks the action button.

pub mod apps;
pub mod background;
pub mod cleanup;
pub mod clutter;
pub mod disks;
pub mod hardware;
pub mod powershell;
pub mod processes;
pub mod protection;
pub mod shell;
pub mod startup;
pub mod system;
pub mod walk;
pub mod winget;
