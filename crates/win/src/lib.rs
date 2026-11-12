//! File system and Windows API access for Scrub My Disks.
//!
//! Everything here is read-only except the functions that change something on purpose:
//! `cleanup::remove_items`, `cleanup::restore`, `system::empty_recycle_bin`,
//! `startup::set_enabled`, `apps::uninstall`, `apps::backup_and_delete_key`,
//! `apps::restore_key`, `winget::install_module`, `winget::update`, `tweaks::apply_*`/`revert_*`,
//! `tweaks::remove_appx`, `toolbox::install`/`upgrade` and `schedule::register`/`unregister`.
//! The elevated helper (`helper`) runs only its typed operations, each checked again. The app calls them
//! only after the user reviews the results and clicks the action button.

pub mod apps;
pub mod background;
pub mod cleanup;
pub mod clutter;
pub mod disks;
#[cfg(windows)]
pub mod elevate;
pub mod env;
pub mod hardware;
#[cfg(windows)]
pub mod helper;
pub mod locks;
pub mod migrate;
pub mod organizer;
#[cfg(windows)]
pub mod perf;
pub mod powershell;
pub mod processes;
pub mod protection;
pub mod quarantine;
#[cfg(windows)]
pub mod schedule;
pub mod shell;
pub mod startup;
pub mod system;
#[cfg(windows)]
pub mod toolbox;
#[cfg(windows)]
pub mod tweaks;
pub mod walk;
#[cfg(windows)]
pub mod watch;
pub mod winget;
