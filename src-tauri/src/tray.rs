use crate::state::{SharedState, WatchStatus};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

const TRAY_GREEN: &[u8] = include_bytes!("../icons/tray-green.png");
const TRAY_ORANGE: &[u8] = include_bytes!("../icons/tray-orange.png");
const TRAY_RED: &[u8] = include_bytes!("../icons/tray-red.png");
const TRAY_GREY: &[u8] = include_bytes!("../icons/tray-grey.png");

/// Determines the worst status across all watches.
pub fn aggregate_status(statuses: &[WatchStatus]) -> WatchStatus {
    if statuses.iter().any(|s| *s == WatchStatus::Error) {
        return WatchStatus::Error;
    }
    if statuses.iter().any(|s| *s == WatchStatus::Warning) {
        return WatchStatus::Warning;
    }
    if statuses.iter().any(|s| *s == WatchStatus::Maintenance) {
        return WatchStatus::Maintenance;
    }
    if statuses.iter().any(|s| *s == WatchStatus::Unknown) {
        return WatchStatus::Unknown;
    }
    WatchStatus::Success
}

/// Returns the icon bytes for a given status.
fn icon_for_status(status: &WatchStatus) -> &'static [u8] {
    match status {
        WatchStatus::Success => TRAY_GREEN,
        WatchStatus::Warning => TRAY_ORANGE,
        WatchStatus::Error => TRAY_RED,
        WatchStatus::Maintenance => TRAY_GREY,
        WatchStatus::Unknown => TRAY_GREY,
    }
}

/// Creates the system tray icon with context menu.
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let refresh_item = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&refresh_item, &quit_item])?;

    TrayIconBuilder::new()
        .icon(Image::from_bytes(TRAY_GREY)?)
        .tooltip("wtch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "refresh" => {
                let _ = app.emit("refresh-requested", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        // Position window above the tray icon, centered horizontally
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let win_width = 320.0_f64;
                        let win_height = 480.0_f64;

                        let tray_pos = rect.position.to_physical::<f64>(scale);
                        let tray_size = rect.size.to_physical::<f64>(scale);

                        let x = tray_pos.x + (tray_size.width / 2.0) - (win_width * scale / 2.0);
                        let y = tray_pos.y - (win_height * scale);

                        let _ = window.set_position(tauri::PhysicalPosition::new(
                            x as i32,
                            y as i32,
                        ));
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Updates the tray icon based on current state.
pub async fn update_tray_icon(app: &AppHandle, state: &SharedState) {
    let state_guard = state.read().await;
    let statuses: Vec<WatchStatus> = state_guard
        .watches
        .iter()
        .map(|w| w.status.clone())
        .collect();
    drop(state_guard);

    let status = if statuses.is_empty() {
        WatchStatus::Unknown
    } else {
        aggregate_status(&statuses)
    };

    let icon_bytes = icon_for_status(&status);
    if let Ok(icon) = Image::from_bytes(icon_bytes) {
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_all_success() {
        let statuses = vec![WatchStatus::Success, WatchStatus::Success];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Success);
    }

    #[test]
    fn aggregate_with_warning() {
        let statuses = vec![WatchStatus::Success, WatchStatus::Warning];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Warning);
    }

    #[test]
    fn aggregate_error_takes_priority() {
        let statuses = vec![
            WatchStatus::Success,
            WatchStatus::Warning,
            WatchStatus::Error,
        ];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Error);
    }

    #[test]
    fn aggregate_unknown_when_no_error_or_warning() {
        let statuses = vec![WatchStatus::Success, WatchStatus::Unknown];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Unknown);
    }

    #[test]
    fn aggregate_maintenance_below_warning() {
        let statuses = vec![WatchStatus::Success, WatchStatus::Maintenance];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Maintenance);
    }

    #[test]
    fn aggregate_warning_over_maintenance() {
        let statuses = vec![WatchStatus::Maintenance, WatchStatus::Warning];
        assert_eq!(aggregate_status(&statuses), WatchStatus::Warning);
    }

    #[test]
    fn aggregate_empty_is_success() {
        assert_eq!(aggregate_status(&[]), WatchStatus::Success);
    }
}
