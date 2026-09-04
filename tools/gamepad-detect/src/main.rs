//! `gamepad-detect` — a small utility to sniff a raw-HID gamepad and print
//! exactly what it reports, so a pad's button/hat/stick layout can be mapped.
//!
//! It was written for the SHANWAN "PS3/PC Gamepad" clone (whose report
//! descriptor defeats Apple's HID element parsing, so GameController.framework
//! and gilrs both come up empty), but it decodes the common 27-byte gamepad
//! report layout and will sniff any raw HID joystick/gamepad:
//!
//!   bytes 0–1 : 13 buttons + 3 pad bits   (bit0 = Button 1 … bit12 = Button 13)
//!   byte 2    : hat switch, LOW nibble    (0=up,2=right,4=down,6=left,15=center)
//!   byte 3    : left stick X  (center 0x7F)
//!   byte 4    : left stick Y  (center 0x7F)
//!
//! Usage:
//!   cargo run --release             # until Ctrl-C
//!   cargo run --release -- 30       # auto-stop after 30 seconds
//!
//! If the pad is in "Xbox 360" mode, macOS hides its raw HID behind Apple's
//! DriverKit driver — press the pad's MODE button to switch it to
//! "PS3/PC Gamepad" mode (expect VID 2563 : PID 0575) and run again.

use std::time::{Duration, Instant};

use hidapi::HidApi;

/// SHANWAN "PS3/PC Gamepad" button order (bit0 = Button 1). Edit for other pads.
const BTN_NAMES: [&str; 13] = [
    "Triangle", "Circle", "Cross", "Square", "L1", "R1", "L2", "R2", "Select",
    "Start", "L3", "R3", "Home",
];

fn main() {
    let max_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let api = HidApi::new().expect("hidapi init");

    println!("=== HID devices ===");
    let mut gamepads = Vec::new();
    for info in api.device_list() {
        let gamepad = info.usage_page() == 0x0001
            && matches!(info.usage(), 0x0004 | 0x0005 | 0x0008);
        println!(
            "  {:04x}:{:04x}  page={:04x} usage={:04x}  iface={}  {} {}",
            info.vendor_id(),
            info.product_id(),
            info.usage_page(),
            info.usage(),
            info.interface_number(),
            info.manufacturer_string().unwrap_or("<?>"),
            info.product_string().unwrap_or("<?>"),
        );
        if gamepad {
            println!("      ^ gamepad-like (usage 0x04/0x05/0x08)");
            gamepads.push(info);
        }
    }
    println!();

    if gamepads.is_empty() {
        eprintln!("! No gamepad-like HID device found.");
        eprintln!("  If the pad is in 'Xbox 360' mode, macOS hides its raw HID —");
        eprintln!("  press MODE to switch to 'PS3/PC Gamepad' mode, then run again.");
        return;
    }

    let info = gamepads[0];
    let dev = match info.open_device(&api) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("! open_device failed: {e}");
            return;
        }
    };
    let _ = dev.set_blocking_mode(false);

    println!("Button legend (SHANWAN PS3/PC Gamepad):");
    println!(
        "  {}",
        BTN_NAMES
            .iter()
            .enumerate()
            .map(|(i, n)| format!("B{}= {}", i + 1, n))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!();
    println!(
        "Opened {:04x}:{:04x} ({}) — press buttons / move the stick.{}",
        info.vendor_id(),
        info.product_id(),
        info.product_string().unwrap_or("?"),
        if max_secs > 0 {
            format!(" Auto-stop in {}s.", max_secs)
        } else {
            " Ctrl-C to stop.".into()
        }
    );
    println!();

    let start = Instant::now();
    let mut last: Option<[u8; 7]> = None;
    loop {
        if max_secs > 0 && start.elapsed() >= Duration::from_secs(max_secs) {
            println!("(reached {max_secs}s limit)");
            break;
        }

        let mut buf = [0u8; 64];
        let n = match dev.read_timeout(&mut buf, 0) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        };
        if n < 7 {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        // Ignore reports whose *input* bytes are unchanged — the trailing vendor
        // bytes tick over on every poll and would otherwise spam the output.
        let key = [buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6]];
        if last == Some(key) {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        last = Some(key);
        decode(&buf[..n.min(27)]);
    }
}

fn decode(r: &[u8]) {
    let buttons = r[0] as u16 | ((r[1] as u16) << 8);
    let mut pressed: Vec<String> = Vec::new();
    for b in 1..=13 {
        if buttons & (1 << (b - 1)) != 0 {
            let name = BTN_NAMES.get(b - 1).copied().unwrap_or("?");
            pressed.push(format!("{}({})", b, name));
        }
    }
    let hat = r[2] & 0x0F;
    let hat_dir = match hat {
        0 => "up",
        1 => "NE",
        2 => "right",
        3 => "SE",
        4 => "down",
        5 => "SW",
        6 => "left",
        7 => "NW",
        15 => "center",
        _ => "?",
    };
    let hex: String = r.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
    println!(
        "btns[{}]  hat={}({})  stick=({},{})\n    raw: {}",
        if pressed.is_empty() {
            "-".into()
        } else {
            pressed.join(", ")
        },
        hat,
        hat_dir,
        r[3],
        r[4],
        hex,
    );
}
