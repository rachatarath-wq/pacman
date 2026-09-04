// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gamepad;
#[cfg(target_os = "macos")]
mod gamepad_hid;

#[tauri::command]
fn gamepad_state(hub: tauri::State<'_, gamepad::GamepadHub>) -> gamepad::GamepadState {
    hub.snapshot()
}

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(gamepad::init(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![gamepad_state])
        .run(tauri::generate_context!())
        .expect("error while running Pac-Man");
}
