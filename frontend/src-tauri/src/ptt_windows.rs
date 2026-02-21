use rdev::{Button, Event, EventType, Key, listen};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Default)]
struct Modifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum MouseButton {
    Middle,
    Back,
    Forward,
}

#[derive(Clone)]
enum PttTarget {
    Key(Key),
    Mouse(MouseButton),
}

#[derive(Clone)]
struct PttCombo {
    modifiers: Modifiers,
    target: PttTarget,
}

fn parse_mouse_button(name: &str) -> Option<MouseButton> {
    match name {
        "MouseMiddle" => Some(MouseButton::Middle),
        "MouseBack" => Some(MouseButton::Back),
        "MouseForward" => Some(MouseButton::Forward),
        _ => None,
    }
}

fn parse_key(name: &str) -> Option<Key> {
    match name {
        "Space" => Some(Key::Space),
        "CapsLock" => Some(Key::CapsLock),
        "Tab" => Some(Key::Tab),
        "Backquote" => Some(Key::BackQuote),
        "Backslash" => Some(Key::BackSlash),
        "BracketLeft" => Some(Key::LeftBracket),
        "BracketRight" => Some(Key::RightBracket),
        "Semicolon" => Some(Key::SemiColon),
        "Quote" => Some(Key::Quote),
        "Comma" => Some(Key::Comma),
        "Period" => Some(Key::Dot),
        "Slash" => Some(Key::Slash),
        "Minus" => Some(Key::Minus),
        "Equal" => Some(Key::Equal),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase() => {
            let key = match s.as_bytes()[0] {
                b'A' => Key::KeyA,
                b'B' => Key::KeyB,
                b'C' => Key::KeyC,
                b'D' => Key::KeyD,
                b'E' => Key::KeyE,
                b'F' => Key::KeyF,
                b'G' => Key::KeyG,
                b'H' => Key::KeyH,
                b'I' => Key::KeyI,
                b'J' => Key::KeyJ,
                b'K' => Key::KeyK,
                b'L' => Key::KeyL,
                b'M' => Key::KeyM,
                b'N' => Key::KeyN,
                b'O' => Key::KeyO,
                b'P' => Key::KeyP,
                b'Q' => Key::KeyQ,
                b'R' => Key::KeyR,
                b'S' => Key::KeyS,
                b'T' => Key::KeyT,
                b'U' => Key::KeyU,
                b'V' => Key::KeyV,
                b'W' => Key::KeyW,
                b'X' => Key::KeyX,
                b'Y' => Key::KeyY,
                b'Z' => Key::KeyZ,
                _ => return None,
            };
            Some(key)
        }
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => {
            let key = match s.as_bytes()[0] {
                b'0' => Key::Num0,
                b'1' => Key::Num1,
                b'2' => Key::Num2,
                b'3' => Key::Num3,
                b'4' => Key::Num4,
                b'5' => Key::Num5,
                b'6' => Key::Num6,
                b'7' => Key::Num7,
                b'8' => Key::Num8,
                b'9' => Key::Num9,
                _ => return None,
            };
            Some(key)
        }
        s if s.starts_with('F') && s[1..].parse::<u32>().is_ok() => {
            let num: u32 = s[1..].parse().unwrap();
            let key = match num {
                1 => Key::F1,
                2 => Key::F2,
                3 => Key::F3,
                4 => Key::F4,
                5 => Key::F5,
                6 => Key::F6,
                7 => Key::F7,
                8 => Key::F8,
                9 => Key::F9,
                10 => Key::F10,
                11 => Key::F11,
                12 => Key::F12,
                _ => return None,
            };
            Some(key)
        }
        _ => None,
    }
}

fn parse_combo(combo: &str) -> Result<PttCombo, String> {
    let parts: Vec<&str> = combo.split('+').collect();
    if parts.is_empty() {
        return Err("Empty key combo".to_string());
    }

    let mut modifiers = Modifiers::default();
    for &part in &parts[..parts.len() - 1] {
        match part {
            "Control" => modifiers.ctrl = true,
            "Shift" => modifiers.shift = true,
            "Alt" => modifiers.alt = true,
            "Meta" => modifiers.meta = true,
            _ => return Err(format!("Unknown modifier: {part}")),
        }
    }

    let target_name = parts[parts.len() - 1];

    // Try mouse button first, then keyboard key
    if let Some(btn) = parse_mouse_button(target_name) {
        Ok(PttCombo {
            modifiers,
            target: PttTarget::Mouse(btn),
        })
    } else if let Some(key) = parse_key(target_name) {
        Ok(PttCombo {
            modifiers,
            target: PttTarget::Key(key),
        })
    } else {
        Err(format!("Unknown key: {target_name}"))
    }
}

fn is_modifier_key(key: Key) -> Option<&'static str> {
    match key {
        Key::ControlLeft | Key::ControlRight => Some("ctrl"),
        Key::ShiftLeft | Key::ShiftRight => Some("shift"),
        Key::Alt | Key::AltGr => Some("alt"),
        Key::MetaLeft | Key::MetaRight => Some("meta"),
        _ => None,
    }
}

fn button_matches(target: &PttTarget, btn: Button) -> bool {
    match target {
        PttTarget::Mouse(MouseButton::Middle) => btn == Button::Middle,
        // XBUTTON1 = back, XBUTTON2 = forward
        PttTarget::Mouse(MouseButton::Back) => matches!(btn, Button::Unknown(1)),
        PttTarget::Mouse(MouseButton::Forward) => matches!(btn, Button::Unknown(2)),
        _ => false,
    }
}

fn key_matches(target: &PttTarget, key: Key) -> bool {
    matches!(target, PttTarget::Key(k) if *k == key)
}

pub fn start(app: tauri::AppHandle, key_name: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap();

    if RUNNING.load(Ordering::SeqCst) {
        STOP_FLAG.store(true, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let combo = parse_combo(key_name)?;

    STOP_FLAG.store(false, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    std::thread::spawn(move || {
        eprintln!("[PTT-rdev] Starting listener");

        let mut held_mods = Modifiers::default();
        let mut target_held = false;
        let mut was_active = false;

        let callback = move |event: Event| {
            if STOP_FLAG.load(Ordering::SeqCst) {
                return;
            }

            match event.event_type {
                EventType::KeyPress(key) => {
                    if let Some(which) = is_modifier_key(key) {
                        match which {
                            "ctrl" => held_mods.ctrl = true,
                            "shift" => held_mods.shift = true,
                            "alt" => held_mods.alt = true,
                            "meta" => held_mods.meta = true,
                            _ => {}
                        }
                    }
                    if key_matches(&combo.target, key) {
                        target_held = true;
                    }
                }
                EventType::KeyRelease(key) => {
                    if let Some(which) = is_modifier_key(key) {
                        match which {
                            "ctrl" => held_mods.ctrl = false,
                            "shift" => held_mods.shift = false,
                            "alt" => held_mods.alt = false,
                            "meta" => held_mods.meta = false,
                            _ => {}
                        }
                    }
                    if key_matches(&combo.target, key) {
                        target_held = false;
                    }
                }
                EventType::ButtonPress(btn) => {
                    if button_matches(&combo.target, btn) {
                        target_held = true;
                    }
                }
                EventType::ButtonRelease(btn) => {
                    if button_matches(&combo.target, btn) {
                        target_held = false;
                    }
                }
                _ => {}
            }

            let mods_ok = (!combo.modifiers.ctrl || held_mods.ctrl)
                && (!combo.modifiers.shift || held_mods.shift)
                && (!combo.modifiers.alt || held_mods.alt)
                && (!combo.modifiers.meta || held_mods.meta);
            let is_active = target_held && mods_ok;

            if is_active != was_active {
                let _ = app.emit("ptt-state", is_active);
                was_active = is_active;
            }
        };

        if let Err(e) = listen(callback) {
            eprintln!("[PTT-rdev] Error: {:?}", e);
        }

        eprintln!("[PTT-rdev] Listener exited");
    });

    Ok(())
}

pub fn stop() {
    let _guard = LOCK.lock().unwrap();
    STOP_FLAG.store(true, Ordering::SeqCst);
    RUNNING.store(false, Ordering::SeqCst);
}

pub fn change_key(app: tauri::AppHandle, key_name: &str) -> Result<(), String> {
    stop();
    std::thread::sleep(std::time::Duration::from_millis(50));
    start(app, key_name)
}
