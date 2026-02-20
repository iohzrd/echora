use evdev::{Device, InputEventKind, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
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
    target: Key,
}

/// Check if an evdev Key is a modifier and return which one.
fn modifier_for_key(key: Key) -> Option<&'static str> {
    match key {
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => Some("ctrl"),
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => Some("shift"),
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => Some("alt"),
        Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => Some("meta"),
        _ => None,
    }
}

/// Map a key name string (from the frontend) to an evdev Key.
fn parse_key(name: &str) -> Option<Key> {
    match name {
        "Space" => Some(Key::KEY_SPACE),
        "CapsLock" => Some(Key::KEY_CAPSLOCK),
        "Tab" => Some(Key::KEY_TAB),
        "Backquote" => Some(Key::KEY_GRAVE),
        "Backslash" => Some(Key::KEY_BACKSLASH),
        "BracketLeft" => Some(Key::KEY_LEFTBRACE),
        "BracketRight" => Some(Key::KEY_RIGHTBRACE),
        "Semicolon" => Some(Key::KEY_SEMICOLON),
        "Quote" => Some(Key::KEY_APOSTROPHE),
        "Comma" => Some(Key::KEY_COMMA),
        "Period" => Some(Key::KEY_DOT),
        "Slash" => Some(Key::KEY_SLASH),
        "Minus" => Some(Key::KEY_MINUS),
        "Equal" => Some(Key::KEY_EQUAL),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase() => {
            let key_code = match s.as_bytes()[0] {
                b'A' => Key::KEY_A,
                b'B' => Key::KEY_B,
                b'C' => Key::KEY_C,
                b'D' => Key::KEY_D,
                b'E' => Key::KEY_E,
                b'F' => Key::KEY_F,
                b'G' => Key::KEY_G,
                b'H' => Key::KEY_H,
                b'I' => Key::KEY_I,
                b'J' => Key::KEY_J,
                b'K' => Key::KEY_K,
                b'L' => Key::KEY_L,
                b'M' => Key::KEY_M,
                b'N' => Key::KEY_N,
                b'O' => Key::KEY_O,
                b'P' => Key::KEY_P,
                b'Q' => Key::KEY_Q,
                b'R' => Key::KEY_R,
                b'S' => Key::KEY_S,
                b'T' => Key::KEY_T,
                b'U' => Key::KEY_U,
                b'V' => Key::KEY_V,
                b'W' => Key::KEY_W,
                b'X' => Key::KEY_X,
                b'Y' => Key::KEY_Y,
                b'Z' => Key::KEY_Z,
                _ => return None,
            };
            Some(key_code)
        }
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => {
            let key_code = match s.as_bytes()[0] {
                b'0' => Key::KEY_0,
                b'1' => Key::KEY_1,
                b'2' => Key::KEY_2,
                b'3' => Key::KEY_3,
                b'4' => Key::KEY_4,
                b'5' => Key::KEY_5,
                b'6' => Key::KEY_6,
                b'7' => Key::KEY_7,
                b'8' => Key::KEY_8,
                b'9' => Key::KEY_9,
                _ => return None,
            };
            Some(key_code)
        }
        s if s.starts_with('F') && s[1..].parse::<u32>().is_ok() => {
            let num: u32 = s[1..].parse().unwrap();
            let key_code = match num {
                1 => Key::KEY_F1,
                2 => Key::KEY_F2,
                3 => Key::KEY_F3,
                4 => Key::KEY_F4,
                5 => Key::KEY_F5,
                6 => Key::KEY_F6,
                7 => Key::KEY_F7,
                8 => Key::KEY_F8,
                9 => Key::KEY_F9,
                10 => Key::KEY_F10,
                11 => Key::KEY_F11,
                12 => Key::KEY_F12,
                _ => return None,
            };
            Some(key_code)
        }
        _ => None,
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

/// Find all keyboard devices that support the target key.
fn find_keyboards(target: Key) -> Vec<Device> {
    evdev::enumerate()
        .filter_map(|(_, device)| {
            let supported = device
                .supported_keys()
                .map_or(false, |keys| keys.contains(target));
            if supported {
                Some(device)
            } else {
                None
            }
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

    let keyboards = find_keyboards(combo.target);
    if keyboards.is_empty() {
        return Err(
            "No keyboard devices found. Ensure the user is in the 'input' group: \
             sudo usermod -aG input $USER (then log out and back in)."
                .to_string(),
        );
    }

    STOP_FLAG.store(false, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    for mut device in keyboards {
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
                            if let InputEventKind::Key(key) = event.kind() {
                                let pressed = event.value() == 1;
                                let released = event.value() == 0;

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
                                if key == combo.target {
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
