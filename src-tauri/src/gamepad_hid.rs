//! macOS raw-HID gamepad reader (fallback for pads that GameController.framework
//! and gilrs cannot read — e.g. the SHANWAN "PS3/PC Gamepad" clone).
//!
//! The SHANWAN clone's report descriptor defeats Apple's IOHIDElement parsing
//! (so gilrs sees `raw_buttons=0 raw_axes=0`) and exposes no GameController
//! profile. But the raw interrupt report is a standard 27-byte layout:
//!
//!   bytes 0–1 : 13 buttons + 3 pad bits  (bit0 = Button 1 … bit12 = Button 13)
//!   byte 2    : hat switch in the LOW nibble (0=N,2=E,4=S,6=W, 15=centered)
//!   byte 3    : left stick X   (8-bit, center 0x7F)
//!   byte 4    : left stick Y   (8-bit, center 0x7F)
//!   byte 5/6  : Z/Rz (right stick) — unused here
//!   bytes 7+  : vendor-specific — unused here
//!
//! This module is macOS-only; keep it gated with `#[cfg(target_os = "macos")]`.

use hidapi::{HidApi, HidDevice};

/// Direction codes must match `gamepad.rs`: 0=Up, 1=Left, 2=Down, 3=Right.
pub const DIR_UP: u8 = 0;
pub const DIR_LEFT: u8 = 1;
pub const DIR_DOWN: u8 = 2;
pub const DIR_RIGHT: u8 = 3;

/// Left-stick deadzone in raw 8-bit units (0..255), center 127.
const STICK_DEADZONE: i32 = 40;

/// If true, a low `ly` (near 0) means "up". Most HID gamepads report up as 0;
/// flip to false if the stick is vertically inverted on a given device.
const STICK_Y_UP_IS_LOW: bool = true;

/// Start/Back button bits, confirmed live against the physical pad (matches the
/// SDL/GLFW `SHANWAN PS3/PC Gamepad` mapping): Button 9 = Select/Back,
/// Button 10 = Start.
const BTN_START: u16 = 1 << 9; // Button 10
const BTN_BACK: u16 = 1 << 8; // Button 9

pub struct RawPad {
    device: HidDevice,
}

pub struct RawState {
    pub hat: Option<u8>, // 4-way decoded (0=up,1=left,2=down,3=right)
    pub hat_raw: u8,     // raw hat nibble 0..15 (15 = centered)
    pub lx: i32,
    pub ly: i32,
    pub buttons: u16,
}

impl RawPad {
    /// Open the first HID device that looks like a gamepad/joystick.
    pub fn open_first() -> Option<RawPad> {
        let api = HidApi::new().ok()?;
        for info in api.device_list() {
            // Generic Desktop page, gamepad-ish usages.
            let is_gamepad = info.usage_page() == 0x0001
                && matches!(info.usage(), 0x0004 | 0x0005 | 0x0008);
            if !is_gamepad {
                continue;
            }
            if let Ok(device) = info.open_device(&api) {
                let _ = device.set_blocking_mode(false);
                return Some(RawPad { device });
            }
        }
        None
    }

    /// Read the latest buffered report (returns None if nothing new).
    pub fn poll(&mut self) -> Option<RawState> {
        let mut buf = [0u8; 64];
        loop {
            match self.device.read_timeout(&mut buf, 0) {
                Ok(0) => return None,
                Ok(n) if n >= 7 => {
                    let hat_raw = (buf[2] & 0x0F) as u8;
                    return Some(RawState {
                        hat: hat_to_dir(hat_raw),
                        hat_raw,
                        lx: buf[3] as i32,
                        ly: buf[4] as i32,
                        buttons: buf[0] as u16 | ((buf[1] as u16) << 8),
                    });
                }
                Ok(_) => continue, // too short, keep draining
                Err(_) => return None,
            }
        }
    }
}

impl RawState {
    /// Direction from the stick (dominant axis past the deadzone), None if neutral.
    pub fn stick_dir(&self) -> Option<u8> {
        let cx = 127;
        let dx = self.lx - cx;
        let dy = if STICK_Y_UP_IS_LOW {
            cx - self.ly // up = low value => positive dy means up
        } else {
            self.ly - cx
        };
        if dx.abs() < STICK_DEADZONE && dy.abs() < STICK_DEADZONE {
            return None;
        }
        if dx.abs() > dy.abs() {
            Some(if dx > 0 { DIR_RIGHT } else { DIR_LEFT })
        } else {
            Some(if dy > 0 { DIR_UP } else { DIR_DOWN })
        }
    }

    pub fn start(&self) -> bool {
        self.buttons & BTN_START != 0
    }

    pub fn back(&self) -> bool {
        self.buttons & BTN_BACK != 0
    }
}

/// Map a raw HID hat-switch nibble to a 4-way direction (diagonals -> None).
fn hat_to_dir(hat: u8) -> Option<u8> {
    match hat {
        0 => Some(DIR_UP),
        2 => Some(DIR_RIGHT),
        4 => Some(DIR_DOWN),
        6 => Some(DIR_LEFT),
        _ => None,
    }
}
