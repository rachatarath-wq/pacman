//! Native gamepad input for the desktop app.
//!
//! The webview (WKWebView on macOS) does not expose the Web Gamepad API, so the
//! desktop build reads controllers natively on a background thread and hands the
//! latest snapshot to the frontend through the `gamepad_state` command.
//!
//! On macOS controllers are read through Apple's **GameController framework**
//! (`GCController`). gilrs is deliberately *not* used there: its IOKit backend
//! enumerates elements with `IOHIDDeviceCopyMatchingElements`, which returns
//! **zero** elements for pads handled by Apple's DriverKit game-controller driver
//! (Xbox 360/One pads and clones such as the SHANWAN). Those devices show up as
//! connected but with `raw_buttons=0 raw_axes=0`. GameController.framework is the
//! supported path for them. Linux/Windows keep the gilrs backend.

use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

/// Analog stick deflection below this magnitude is treated as centered.
const STICK_DEADZONE: f32 = 0.35;

/// Direction codes match the WASM core (0 up, 1 left, 2 down, 3 right);
/// `None` while the stick/d-pad is centered.
///
/// `kind` tells the frontend how to interpret the snapshot:
/// 0 = no pad, 1 = raw-HID (frontend maps `buttons`/`hat`/`stick_*`), 2 = GC, 3 = gilrs.
#[derive(Clone, Copy, Default, Serialize, PartialEq)]
pub struct GamepadState {
    pub dir: Option<u8>,
    pub start: bool,
    pub back: bool,
    pub connected: bool,
    pub kind: u8,
    pub buttons: u16,
    pub hat: u8,
    pub stick_x: i32,
    pub stick_y: i32,
}

/// Shared, thread-safe snapshot read by the `gamepad_state` command.
pub struct GamepadHub {
    shared: Arc<Mutex<GamepadState>>,
}

impl GamepadHub {
    pub fn snapshot(&self) -> GamepadState {
        *self.shared.lock().unwrap()
    }
}

/// Spawn the platform input loop and return a hub the command can read. The loop
/// pushes fresh snapshots to the frontend as `gamepad-state` events.
pub fn init(app: AppHandle) -> GamepadHub {
    let shared = Arc::new(Mutex::new(GamepadState::default()));
    imp::run(Arc::clone(&shared), app);
    GamepadHub { shared }
}

// ---- macOS: GameController.framework -------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use super::{GamepadState, STICK_DEADZONE};
    use objc2::rc::autoreleasepool;
    use objc2_game_controller::{GCController, GCExtendedGamepad};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    pub fn run(shared: Arc<Mutex<GamepadState>>, app: tauri::AppHandle) {
        use tauri::Emitter;
        // macOS 11.3+ defaults to NOT delivering game-controller events unless
        // the app is frontmost — so the moment the window loses focus (e.g.
        // while watching the terminal), every element reads neutral. Force
        // background monitoring on so input keeps flowing.
        unsafe { GCController::setShouldMonitorBackgroundEvents(true) };

        std::thread::spawn(move || {
            // Raw-HID fallback for pads GameController.framework can't read
            // (e.g. the SHANWAN clone). Its reports are merged below; official
            // controllers keep the GC path.
            let mut raw = crate::gamepad_hid::RawPad::open_first();
            let mut last_raw = std::time::Instant::now();
            let mut was_connected = false;
            let mut tick: u64 = 0;
            let mut last_emitted = GamepadState::default();
            loop {
                // GCController reads must happen inside an autorelease pool so
                // the ObjC temporaries are released promptly.
                let (mut state, extended, debug) = autoreleasepool(|_pool| {
                    // `controllers()` lists every attached controller; reading
                    // its elements is thread-safe.
                    let controllers = unsafe { GCController::controllers() };
                    let count = controllers.count();
                    let mut state = GamepadState {
                        connected: count > 0,
                        kind: if count > 0 { 2 } else { 0 },
                        ..Default::default()
                    };
                    let mut extended = false;
                    let mut debug = None;

                    // Use the first controller with an extended profile
                    // (d-pad + analog stick + Menu/Options buttons).
                    for i in 0..count {
                        let controller = controllers.objectAtIndex(i);
                        let Some(pad) = (unsafe { controller.extendedGamepad() }) else {
                            continue;
                        };
                        extended = true;
                        state.dir = direction(&pad);
                        state.start = unsafe { pad.buttonMenu().isPressed() };
                        state.back = match unsafe { pad.buttonOptions() } {
                            Some(b) => unsafe { b.isPressed() },
                            None => false,
                        };
                        // Dump the raw elements once per second so we can see
                        // exactly what the framework reports while playing.
                        if tick % 60 == 0 {
                            debug = Some(dump_pad(&pad));
                        }
                        break;
                    }
                    (state, extended, debug)
                });

                // Merge the raw-HID snapshot: fall back to it for direction
                // when GC reports none, and OR its Start/Back buttons in.
                if let Some(pad) = raw.as_mut() {
                    if let Some(s) = pad.poll() {
                        last_raw = std::time::Instant::now();
                        state.kind = 1;
                        state.buttons = s.buttons;
                        state.hat = s.hat_raw;
                        state.stick_x = s.lx;
                        state.stick_y = s.ly;
                        if state.dir.is_none() {
                            state.dir = s.hat.or_else(|| s.stick_dir());
                        }
                        state.start |= s.start();
                        state.back |= s.back();
                    }
                }
                // A raw pad keeps `connected` true only while reports flow.
                if raw.is_some() && last_raw.elapsed() < Duration::from_secs(1) {
                    state.connected = true;
                    if state.kind == 0 {
                        state.kind = 1;
                    }
                }

                if state.connected != was_connected {
                    was_connected = state.connected;
                    if state.connected {
                        eprintln!(
                            "[gamepad] connected via GameController.framework (extended profile: {extended})"
                        );
                    } else {
                        eprintln!("[gamepad] disconnected");
                    }
                }
                if let Some(line) = debug {
                    eprintln!("[gamepad] {line}");
                }

                *shared.lock().unwrap() = state;
                if state != last_emitted {
                    let _ = app.emit("gamepad-state", &state);
                    last_emitted = state;
                }
                tick += 1;
                std::thread::sleep(Duration::from_millis(8)); // ~120 Hz
            }
        });
    }

    fn dump_pad(pad: &GCExtendedGamepad) -> String {
        let dpad = unsafe { pad.dpad() };
        let stick = unsafe { pad.leftThumbstick() };
        let options = match unsafe { pad.buttonOptions() } {
            Some(b) => unsafe { b.isPressed() },
            None => false,
        };
        format!(
            "dpad U={} D={} L={} R={} | stick x={:.2} y={:.2} | menu={} options={}",
            unsafe { dpad.up().isPressed() },
            unsafe { dpad.down().isPressed() },
            unsafe { dpad.left().isPressed() },
            unsafe { dpad.right().isPressed() },
            unsafe { stick.xAxis().value() },
            unsafe { stick.yAxis().value() },
            unsafe { pad.buttonMenu().isPressed() },
            options,
        )
    }

    fn direction(pad: &GCExtendedGamepad) -> Option<u8> {
        // 1. Digital d-pad.
        let dpad = unsafe { pad.dpad() };
        if unsafe { dpad.up().isPressed() } {
            return Some(0);
        }
        if unsafe { dpad.down().isPressed() } {
            return Some(2);
        }
        if unsafe { dpad.left().isPressed() } {
            return Some(1);
        }
        if unsafe { dpad.right().isPressed() } {
            return Some(3);
        }

        // 2. Left analog stick. GameController.framework reports the Y axis
        // up-positive (+1 = up), matching the game's direction codes.
        let stick = unsafe { pad.leftThumbstick() };
        let x = unsafe { stick.xAxis().value() };
        let y = unsafe { stick.yAxis().value() };
        if x.abs() > STICK_DEADZONE || y.abs() > STICK_DEADZONE {
            if x.abs() >= y.abs() {
                Some(if x > 0.0 { 3 } else { 1 })
            } else {
                Some(if y > 0.0 { 0 } else { 2 })
            }
        } else {
            None
        }
    }
}

// ---- other platforms: gilrs ---------------------------------------------

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{GamepadState, STICK_DEADZONE};
    use gilrs::{Axis, Button, Gamepad, Gilrs};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const HAT_THRESHOLD: f32 = 0.5;

    pub fn run(shared: Arc<Mutex<GamepadState>>, app: tauri::AppHandle) {
        use tauri::Emitter;
        std::thread::spawn(move || {
            let mut gilrs = match Gilrs::new() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("[gamepad] gilrs failed to init: {e}");
                    return;
                }
            };
            let mut was_connected = false;
            let mut last_emitted = GamepadState::default();
            loop {
                // Drain events (also refreshes gilrs' internal state), then
                // read the latest snapshot.
                while gilrs.next_event().is_some() {}
                let state = refresh(&gilrs);

                if let Some((_id, pad)) = gilrs.gamepads().next() {
                    if !was_connected {
                        was_connected = true;
                        eprintln!(
                            "[gamepad] connected: name=\"{}\" mapping={:?}",
                            pad.name(),
                            pad.mapping_source(),
                        );
                    }
                } else {
                    was_connected = false;
                }

                *shared.lock().unwrap() = state;
                if state != last_emitted {
                    let _ = app.emit("gamepad-state", &state);
                    last_emitted = state;
                }
                std::thread::sleep(Duration::from_millis(8)); // ~120 Hz
            }
        });
    }

    fn refresh(gilrs: &Gilrs) -> GamepadState {
        let mut state = GamepadState::default();
        if let Some((_id, pad)) = gilrs.gamepads().next() {
            state.connected = true;
            state.kind = 3;
            state.dir = direction(&pad);
            state.start = pad.is_pressed(Button::Start);
            state.back = pad.is_pressed(Button::Select);
        }
        state
    }

    fn direction(pad: &Gamepad<'_>) -> Option<u8> {
        // 1. D-pad buttons.
        if pad.is_pressed(Button::DPadUp) {
            return Some(0);
        }
        if pad.is_pressed(Button::DPadDown) {
            return Some(2);
        }
        if pad.is_pressed(Button::DPadLeft) {
            return Some(1);
        }
        if pad.is_pressed(Button::DPadRight) {
            return Some(3);
        }

        // 2. D-pad reported as a hat switch (saturated -1/0/1).
        let dx = pad.value(Axis::DPadX);
        let dy = pad.value(Axis::DPadY);
        if dx.abs() > HAT_THRESHOLD || dy.abs() > HAT_THRESHOLD {
            if dx.abs() >= dy.abs() {
                return Some(if dx > 0.0 { 3 } else { 1 });
            }
            return Some(if dy > 0.0 { 0 } else { 2 });
        }

        // 3. Left analog stick (gilrs reports Y up-positive).
        let x = pad.value(Axis::LeftStickX);
        let y = pad.value(Axis::LeftStickY);
        if x.abs() > STICK_DEADZONE || y.abs() > STICK_DEADZONE {
            if x.abs() >= y.abs() {
                Some(if x > 0.0 { 3 } else { 1 })
            } else {
                Some(if y > 0.0 { 0 } else { 2 })
            }
        } else {
            None
        }
    }
}
