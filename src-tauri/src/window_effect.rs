//! Window background material: Mica on Windows 11, acrylic on Windows 10.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowEffect {
    Mica,
    Acrylic,
    None,
}

#[cfg(windows)]
pub fn apply(window: &tauri::WebviewWindow) -> WindowEffect {
    use window_vibrancy::{apply_acrylic, apply_mica};

    // `None` lets Mica follow the window theme, which the UI sets with `setTheme`.
    match apply_mica(window, None) {
        Ok(()) => WindowEffect::Mica,
        Err(mica_err) => {
            log::info!("Mica unavailable ({mica_err}), trying acrylic");
            match apply_acrylic(window, None) {
                Ok(()) => WindowEffect::Acrylic,
                Err(err) => {
                    log::warn!("no window effect available: {err}");
                    WindowEffect::None
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub fn apply(_window: &tauri::WebviewWindow) -> WindowEffect {
    WindowEffect::None
}
