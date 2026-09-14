//! Scripted driving for development and review (native only). Example:
//!
//! ```text
//! STARDUST_KEYS="1:space;3:space;4:right*0.5" STARDUST_SHOTS="2:title.png;6:play.png" cargo run
//! ```
//!
//! `F12` saves a screenshot at any time.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

pub fn plugin(app: &mut App) {
    app.insert_resource(Script::from_env()).add_systems(
        PreUpdate,
        (drive_keys, take_shots).after(bevy::input::InputSystems),
    );
}

#[derive(Resource, Default)]
struct Script {
    keys: Vec<KeyPress>,
    shots: Vec<(f32, PathBuf)>,
    /// Keys currently held by the script, released when their time is up.
    held: Vec<(f32, KeyCode)>,
    shot_count: u32,
}

struct KeyPress {
    at: f32,
    key: KeyCode,
    hold: f32,
}

impl Script {
    fn from_env() -> Self {
        let keys = std::env::var("STARDUST_KEYS")
            .ok()
            .map(|s| parse_keys(&s))
            .unwrap_or_default();
        let shots = std::env::var("STARDUST_SHOTS")
            .ok()
            .map(|s| {
                s.split(';')
                    .filter_map(|e| e.split_once(':'))
                    .filter_map(|(t, p)| Some((t.parse().ok()?, PathBuf::from(p))))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            keys,
            shots,
            held: Vec::new(),
            shot_count: 0,
        }
    }
}

fn parse_keys(spec: &str) -> Vec<KeyPress> {
    spec.split(';')
        .filter_map(|entry| {
            let (at, rest) = entry.split_once(':')?;
            let (name, hold) = rest
                .split_once('*')
                .map_or((rest, 0.0), |(n, h)| (n, h.parse().unwrap_or(0.0)));
            Some(KeyPress {
                at: at.parse().ok()?,
                key: key_code(name)?,
                hold,
            })
        })
        .collect()
}

fn key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "space" => KeyCode::Space,
        "enter" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Escape,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "backspace" => KeyCode::Backspace,
        "f12" => KeyCode::F12,
        single if single.len() == 1 => {
            let c = single.chars().next()?.to_ascii_uppercase();
            match c {
                'A'..='Z' => letter_key(c),
                '0'..='9' => digit_key(c),
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn letter_key(c: char) -> KeyCode {
    const KEYS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    KEYS[(c as u8 - b'A') as usize]
}

fn digit_key(c: char) -> KeyCode {
    const KEYS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    KEYS[(c as u8 - b'0') as usize]
}

fn drive_keys(time: Res<Time>, mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let now = time.elapsed_secs();
    let script = &mut *script;
    script.held.retain(|(until, key)| {
        let done = now >= *until;
        if done {
            keys.release(*key);
        }
        !done
    });
    let due: Vec<KeyPress> = script.keys.extract_if(.., |k| k.at <= now).collect();
    for press in due {
        keys.press(press.key);
        // A plain tap is released next frame; a hold lasts `hold` seconds.
        script.held.push((now + press.hold, press.key));
    }
}

fn take_shots(
    mut commands: Commands,
    time: Res<Time>,
    mut script: ResMut<Script>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let now = time.elapsed_secs();
    let mut due: Vec<PathBuf> = script
        .shots
        .extract_if(.., |(at, _)| *at <= now)
        .map(|(_, p)| p)
        .collect();
    if keys.just_pressed(KeyCode::F12) {
        script.shot_count += 1;
        due.push(PathBuf::from(format!(
            "screenshot-{}.png",
            script.shot_count
        )));
    }
    for path in due {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}
