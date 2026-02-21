use evdev::{Device, EventSummary, KeyCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
// Guards against concurrent start/stop races
static LOCK: Mutex<()> = Mutex::new(());

/// Modifier flags (bitmask).
#[derive(Clone, Copy, Default)]
struct Modifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
}

/// Parsed key combo: required modifiers + target key.
#[derive(Clone)]
struct KeyCombo {
    modifiers: Modifiers,
    target: KeyCode,
}

/// Check if an evdev Key is a modifier and return which one.
fn modifier_for_key(key: KeyCode) -> Option<&'static str> {
    match key {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => Some("ctrl"),
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => Some("shift"),
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => Some("alt"),
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => Some("meta"),
        _ => None,
    }
}

/// Map a key name string (from the frontend) to an evdev Key.
fn parse_key(name: &str) -> Option<KeyCode> {
    match name {
        // Mouse buttons
        "MouseMiddle" => Some(KeyCode::BTN_MIDDLE),
        "MouseBack" => Some(KeyCode::BTN_SIDE),
        "MouseForward" => Some(KeyCode::BTN_EXTRA),
        // Keyboard keys
        "Space" => Some(KeyCode::KEY_SPACE),
        "CapsLock" => Some(KeyCode::KEY_CAPSLOCK),
        "Tab" => Some(KeyCode::KEY_TAB),
        "Backquote" => Some(KeyCode::KEY_GRAVE),
        "Backslash" => Some(KeyCode::KEY_BACKSLASH),
        "BracketLeft" => Some(KeyCode::KEY_LEFTBRACE),
        "BracketRight" => Some(KeyCode::KEY_RIGHTBRACE),
        "Semicolon" => Some(KeyCode::KEY_SEMICOLON),
        "Quote" => Some(KeyCode::KEY_APOSTROPHE),
        "Comma" => Some(KeyCode::KEY_COMMA),
        "Period" => Some(KeyCode::KEY_DOT),
        "Slash" => Some(KeyCode::KEY_SLASH),
        "Minus" => Some(KeyCode::KEY_MINUS),
        "Equal" => Some(KeyCode::KEY_EQUAL),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase() => {
            let key_code = match s.as_bytes()[0] {
                b'A' => KeyCode::KEY_A,
                b'B' => KeyCode::KEY_B,
                b'C' => KeyCode::KEY_C,
                b'D' => KeyCode::KEY_D,
                b'E' => KeyCode::KEY_E,
                b'F' => KeyCode::KEY_F,
                b'G' => KeyCode::KEY_G,
                b'H' => KeyCode::KEY_H,
                b'I' => KeyCode::KEY_I,
                b'J' => KeyCode::KEY_J,
                b'K' => KeyCode::KEY_K,
                b'L' => KeyCode::KEY_L,
                b'M' => KeyCode::KEY_M,
                b'N' => KeyCode::KEY_N,
                b'O' => KeyCode::KEY_O,
                b'P' => KeyCode::KEY_P,
                b'Q' => KeyCode::KEY_Q,
                b'R' => KeyCode::KEY_R,
                b'S' => KeyCode::KEY_S,
                b'T' => KeyCode::KEY_T,
                b'U' => KeyCode::KEY_U,
                b'V' => KeyCode::KEY_V,
                b'W' => KeyCode::KEY_W,
                b'X' => KeyCode::KEY_X,
                b'Y' => KeyCode::KEY_Y,
                b'Z' => KeyCode::KEY_Z,
                _ => return None,
            };
            Some(key_code)
        }
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => {
            let key_code = match s.as_bytes()[0] {
                b'0' => KeyCode::KEY_0,
                b'1' => KeyCode::KEY_1,
                b'2' => KeyCode::KEY_2,
                b'3' => KeyCode::KEY_3,
                b'4' => KeyCode::KEY_4,
                b'5' => KeyCode::KEY_5,
                b'6' => KeyCode::KEY_6,
                b'7' => KeyCode::KEY_7,
                b'8' => KeyCode::KEY_8,
                b'9' => KeyCode::KEY_9,
                _ => return None,
            };
            Some(key_code)
        }
        s if s.starts_with('F') && s[1..].parse::<u32>().is_ok() => {
            let num: u32 = s[1..].parse().unwrap();
            let key_code = match num {
                1 => KeyCode::KEY_F1,
                2 => KeyCode::KEY_F2,
                3 => KeyCode::KEY_F3,
                4 => KeyCode::KEY_F4,
                5 => KeyCode::KEY_F5,
                6 => KeyCode::KEY_F6,
                7 => KeyCode::KEY_F7,
                8 => KeyCode::KEY_F8,
                9 => KeyCode::KEY_F9,
                10 => KeyCode::KEY_F10,
                11 => KeyCode::KEY_F11,
                12 => KeyCode::KEY_F12,
                _ => return None,
            };
            Some(key_code)
        }
        _ => None,
    }
}

/// Check if a KeyCode matches the target, accounting for alternative mouse button codes.
/// Some mice report BTN_BACK/BTN_FORWARD instead of BTN_SIDE/BTN_EXTRA for the same buttons.
fn key_matches_target(key: KeyCode, target: KeyCode) -> bool {
    if key == target {
        return true;
    }
    match target {
        KeyCode::BTN_SIDE => key == KeyCode::BTN_BACK,
        KeyCode::BTN_EXTRA => key == KeyCode::BTN_FORWARD,
        _ => false,
    }
}

/// Parse a combo string like "Control+Shift+Space" into a KeyCombo.
fn parse_combo(combo: &str) -> Result<KeyCombo, String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("Empty key combo".to_string());
    }

    let mut modifiers = Modifiers::default();

    // All parts except the last are modifiers
    for &part in &parts[..parts.len() - 1] {
        match part {
            "Control" => modifiers.ctrl = true,
            "Shift" => modifiers.shift = true,
            "Alt" => modifiers.alt = true,
            "Meta" => modifiers.meta = true,
            _ => return Err(format!("Unknown modifier: {part}")),
        }
    }

    // Last part is the target key
    let target_name = parts[parts.len() - 1];
    let target = parse_key(target_name).ok_or_else(|| format!("Unknown key: {target_name}"))?;

    Ok(KeyCombo { modifiers, target })
}

/// Find all input devices that support the target key (keyboards and mice).
fn find_devices(target: KeyCode) -> Vec<Device> {
    // Some mice report alternative button codes for the same physical button
    let targets: Vec<KeyCode> = match target {
        KeyCode::BTN_SIDE => vec![KeyCode::BTN_SIDE, KeyCode::BTN_BACK],
        KeyCode::BTN_EXTRA => vec![KeyCode::BTN_EXTRA, KeyCode::BTN_FORWARD],
        other => vec![other],
    };

    evdev::enumerate()
        .filter_map(|(_, device)| {
            let supported = device
                .supported_keys()
                .map_or(false, |keys| targets.iter().any(|t| keys.contains(*t)));
            if supported { Some(device) } else { None }
        })
        .collect()
}

/// Start the evdev PTT listener. Spawns background threads that emit
/// `ptt-state` events to the Tauri frontend.
pub fn start(app: tauri::AppHandle, key_name: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap();

    // Stop any existing listener first
    if RUNNING.load(Ordering::SeqCst) {
        STOP_FLAG.store(true, Ordering::SeqCst);
        // Give threads a moment to exit
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let combo = parse_combo(key_name)?;

    let devices = find_devices(combo.target);
    if devices.is_empty() {
        return Err("No input devices found supporting the target key. \
             Ensure the user is in the 'input' group: \
             sudo usermod -aG input $USER (then log out and back in)."
            .to_string());
    }

    STOP_FLAG.store(false, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    for mut device in devices {
        let app = app.clone();
        let stop = &STOP_FLAG;
        let name = device.name().unwrap_or("unknown").to_string();
        let combo = combo.clone();

        std::thread::spawn(move || {
            eprintln!("[PTT-evdev] Monitoring: {name}");

            // Track live modifier and target key state
            let mut held_mods = Modifiers::default();
            let mut target_held = false;
            let mut was_active = false;

            loop {
                if stop.load(Ordering::SeqCst) {
                    eprintln!("[PTT-evdev] Stopping: {name}");
                    break;
                }

                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if let EventSummary::Key(_, key, value) = event.destructure() {
                                let pressed = value == 1;
                                let released = value == 0;

                                if !pressed && !released {
                                    continue; // skip repeat (2)
                                }

                                // Update modifier state
                                if let Some(which) = modifier_for_key(key) {
                                    match which {
                                        "ctrl" => {
                                            if pressed {
                                                held_mods.ctrl = true;
                                            } else if released {
                                                held_mods.ctrl = false;
                                            }
                                        }
                                        "shift" => {
                                            if pressed {
                                                held_mods.shift = true;
                                            } else if released {
                                                held_mods.shift = false;
                                            }
                                        }
                                        "alt" => {
                                            if pressed {
                                                held_mods.alt = true;
                                            } else if released {
                                                held_mods.alt = false;
                                            }
                                        }
                                        "meta" => {
                                            if pressed {
                                                held_mods.meta = true;
                                            } else if released {
                                                held_mods.meta = false;
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // Update target key state
                                if key_matches_target(key, combo.target) {
                                    if pressed {
                                        target_held = true;
                                    } else if released {
                                        target_held = false;
                                    }
                                }

                                // Check if the full combo is satisfied
                                let mods_ok = (!combo.modifiers.ctrl || held_mods.ctrl)
                                    && (!combo.modifiers.shift || held_mods.shift)
                                    && (!combo.modifiers.alt || held_mods.alt)
                                    && (!combo.modifiers.meta || held_mods.meta);
                                let is_active = target_held && mods_ok;

                                if is_active != was_active {
                                    let _ = app.emit("ptt-state", is_active);
                                    was_active = is_active;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[PTT-evdev] Device error ({name}): {e}");
                        break;
                    }
                }
            }
        });
    }

    Ok(())
}

/// Stop all evdev listener threads.
pub fn stop() {
    let _guard = LOCK.lock().unwrap();
    STOP_FLAG.store(true, Ordering::SeqCst);
    RUNNING.store(false, Ordering::SeqCst);
}

/// Change the PTT key by restarting the listener.
pub fn change_key(app: tauri::AppHandle, key_name: &str) -> Result<(), String> {
    stop();
    std::thread::sleep(std::time::Duration::from_millis(50));
    start(app, key_name)
}
