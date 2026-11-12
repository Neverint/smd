//! Windows notifications and the few backend texts (tray menu), in English or French.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::state::{AppState, lock};

#[derive(Clone, Copy)]
pub enum Text {
    TrayOpen,
    TraySmartCare,
    TrayQuit,
    TaskDone,
    SmartCareDone,
    SmartCareFailed,
    GuardTitle,
}

fn french(state: &AppState) -> bool {
    lock(&state.settings).is_ok_and(|s| s.language.starts_with("fr"))
}

pub fn text(state: &AppState, t: Text) -> &'static str {
    let fr = french(state);
    match (t, fr) {
        (Text::TrayOpen, false) => "Open Scrub My Disks",
        (Text::TrayOpen, true) => "Ouvrir Scrub My Disks",
        (Text::TraySmartCare, false) => "Run Smart Care",
        (Text::TraySmartCare, true) => "Lancer l'entretien express",
        (Text::TrayQuit, false) => "Quit",
        (Text::TrayQuit, true) => "Quitter",
        (Text::TaskDone, false) => "Your task is finished.",
        (Text::TaskDone, true) => "Votre tâche est terminée.",
        (Text::SmartCareDone, false) => "Smart Care: {score}/100. Open the app to see the fixes.",
        (Text::SmartCareDone, true) => {
            "Entretien express : {score}/100. Ouvrez l'application pour voir les corrections."
        }
        (Text::SmartCareFailed, false) => "Smart Care could not run.",
        (Text::SmartCareFailed, true) => "L'entretien express n'a pas pu s'exécuter.",
        (Text::GuardTitle, false) => "A game is on the wrong drive",
        (Text::GuardTitle, true) => "Un jeu est sur le mauvais disque",
    }
}

/// "{name} ({size}) was installed on {drive}. Your layout puts it on {target}: open Drive Organizer to move it."
pub fn guard_body(state: &AppState, name: &str, size: &str, drive: &str, target: &str) -> String {
    if french(state) {
        format!(
            "{name} ({size}) est installé sur {drive}. Votre organisation le prévoit sur {target} : ouvrez Organisation des disques pour le déplacer."
        )
    } else {
        format!(
            "{name} ({size}) was installed on {drive}. Your layout puts it on {target}: open Drive Organizer to move it."
        )
    }
}

pub fn show(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}
