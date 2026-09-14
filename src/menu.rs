//! Title, story, instructions, password entry, level intro and victory
//! screens. Everything is drawn in world space so it scales with the play
//! field.

use crate::AppState;
use crate::assets::{GameAssets, LevelAsset};
use crate::audio::PlaySound;
use crate::grid::Z_OVERLAY;
use crate::play::{Pending, Source};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Progress>()
        .init_resource::<PasswordEntry>()
        .add_systems(OnEnter(AppState::Title), title_screen)
        .add_systems(
            OnEnter(AppState::Story),
            |c: Commands, a: Res<GameAssets>, s: MessageWriter<PlaySound>| {
                picture_screen(c, a.story.clone(), s);
            },
        )
        .add_systems(
            OnEnter(AppState::Instructions),
            |c: Commands, a: Res<GameAssets>, s: MessageWriter<PlaySound>| {
                picture_screen(c, a.instructions.clone(), s);
            },
        )
        .add_systems(OnEnter(AppState::Password), password_screen)
        .add_systems(OnEnter(AppState::LevelIntro), level_intro)
        .add_systems(OnEnter(AppState::Victory), victory_screen)
        .add_systems(Update, title_keys.run_if(in_state(AppState::Title)))
        .add_systems(
            Update,
            back_to_title.run_if(
                in_state(AppState::Story)
                    .or_else(in_state(AppState::Instructions))
                    .or_else(in_state(AppState::Victory)),
            ),
        )
        .add_systems(Update, password_keys.run_if(in_state(AppState::Password)))
        .add_systems(Update, intro_keys.run_if(in_state(AppState::LevelIntro)));
}

/// Campaign level currently reached (0-based).
#[derive(Resource, Default)]
pub struct Progress {
    pub level: usize,
}

/// Everything needed to kick off a campaign level.
#[derive(SystemParam)]
pub struct StartLevel<'w, 's> {
    commands: Commands<'w, 's>,
    next: ResMut<'w, NextState<AppState>>,
    progress: ResMut<'w, Progress>,
    assets: Res<'w, GameAssets>,
    levels: Res<'w, Assets<LevelAsset>>,
}

impl StartLevel<'_, '_> {
    /// Jump to the intro of campaign level `index`; false if it isn't loaded.
    pub fn start(&mut self, index: usize) -> bool {
        let Some(level) = self
            .assets
            .levels
            .get(index)
            .and_then(|h| self.levels.get(h))
        else {
            return false;
        };
        self.progress.level = index;
        self.commands.insert_resource(Pending {
            level: level.0.clone(),
            source: Source::Campaign(index),
        });
        self.next.set(AppState::LevelIntro);
        true
    }

    pub fn index_with_password(&self, password: &str) -> Option<usize> {
        self.assets.levels.iter().position(|h| {
            self.levels.get(h).and_then(|l| l.0.password.as_deref()) == Some(password)
        })
    }
}

#[derive(Resource, Default)]
struct PasswordEntry(String);

#[derive(Component)]
struct PasswordText;

const CYAN: Color = Color::srgb(0.4, 1.0, 1.0);
const GREEN: Color = Color::srgb(0.3, 1.0, 0.3);

fn text(
    value: impl Into<String>,
    size: f32,
    color: Color,
    at: Vec2,
    state: AppState,
) -> impl Bundle {
    (
        Text2d::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::justify(Justify::Center),
        Transform::from_translation(at.extend(Z_OVERLAY)),
        DespawnOnExit(state),
    )
}

fn title_screen(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut sounds: MessageWriter<PlaySound>,
) {
    commands.spawn((
        Sprite::from_image(assets.logo.clone()),
        Transform::from_xyz(0.0, 120.0, Z_OVERLAY),
        DespawnOnExit(AppState::Title),
    ));
    commands.spawn(text(
        "space  new game\np  start at a password\ni  instructions\ns  the story\ne  level editor",
        22.0,
        CYAN,
        Vec2::new(0.0, -60.0),
        AppState::Title,
    ));
    sounds.write(PlaySound("program_s_begun"));
}

fn title_keys(keys: Res<ButtonInput<KeyCode>>, mut start: StartLevel) {
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]) {
        start.start(0);
    } else if keys.just_pressed(KeyCode::KeyP) {
        start.next.set(AppState::Password);
    } else if keys.just_pressed(KeyCode::KeyI) {
        start.next.set(AppState::Instructions);
    } else if keys.just_pressed(KeyCode::KeyS) {
        start.next.set(AppState::Story);
    } else if keys.just_pressed(KeyCode::KeyE) {
        start.next.set(AppState::Editor);
    }
}

fn picture_screen(
    mut commands: Commands,
    image: Handle<Image>,
    mut sounds: MessageWriter<PlaySound>,
) {
    // The original screens are 647×460; scale to fit the 640-wide field.
    commands.spawn((
        Sprite::from_image(image),
        Transform::from_xyz(0.0, 0.0, Z_OVERLAY).with_scale(Vec3::splat(640.0 / 647.0)),
        DespawnOnEnter(AppState::Title),
    ));
    sounds.write(PlaySound("information"));
}

fn back_to_title(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.get_just_pressed().next().is_some() || mouse.just_pressed(MouseButton::Left) {
        next.set(AppState::Title);
    }
}

fn password_screen(mut commands: Commands, mut entry: ResMut<PasswordEntry>) {
    entry.0.clear();
    commands.spawn(text(
        "Enter the password of the level\nwhere you want to begin",
        20.0,
        CYAN,
        Vec2::new(0.0, 70.0),
        AppState::Password,
    ));
    commands.spawn((
        text("_", 32.0, GREEN, Vec2::new(0.0, 0.0), AppState::Password),
        PasswordText,
    ));
    commands.spawn(text(
        "return  begin      esc  back",
        16.0,
        Color::srgb(0.5, 0.5, 0.6),
        Vec2::new(0.0, -80.0),
        AppState::Password,
    ));
}

fn password_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut entry: ResMut<PasswordEntry>,
    mut display: Single<&mut Text2d, With<PasswordText>>,
    mut start: StartLevel,
    mut sounds: MessageWriter<PlaySound>,
) {
    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Escape => start.next.set(AppState::Title),
            KeyCode::Backspace => {
                entry.0.pop();
            }
            KeyCode::Enter => {
                let started = start
                    .index_with_password(&entry.0)
                    .is_some_and(|i| start.start(i));
                if !started {
                    sounds.write(PlaySound("password_no_good"));
                    entry.0.clear();
                }
            }
            key => {
                if let Some(c) = letter(*key) {
                    if entry.0.len() < 12 {
                        entry.0.push(c);
                    }
                }
            }
        }
    }
    display.0 = format!("{}_", entry.0);
}

fn letter(key: KeyCode) -> Option<char> {
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
    KEYS.iter()
        .position(|k| *k == key)
        .map(|i| char::from(b'A' + i as u8))
}

fn level_intro(
    mut commands: Commands,
    pending: Res<Pending>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let title = match pending.source {
        Source::Campaign(_) => format!("Level {}", pending.level.name),
        Source::Editor => format!("Testing: {}", pending.level.name),
    };
    commands.spawn(text(
        title,
        40.0,
        GREEN,
        Vec2::new(0.0, 40.0),
        AppState::LevelIntro,
    ));
    if let Some(password) = &pending.level.password {
        commands.spawn(text(
            format!("password  {password}"),
            22.0,
            CYAN,
            Vec2::new(0.0, -20.0),
            AppState::LevelIntro,
        ));
    }
    commands.spawn(text(
        "click mouse or hit spacebar to continue",
        16.0,
        Color::srgb(0.5, 0.5, 0.6),
        Vec2::new(0.0, -100.0),
        AppState::LevelIntro,
    ));
    sounds.write(PlaySound("level_screen"));
}

fn intro_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut next: ResMut<NextState<AppState>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    } else if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter])
        || mouse.just_pressed(MouseButton::Left)
    {
        sounds.write(PlaySound("begin_playing"));
        next.set(AppState::Playing);
    }
}

fn victory_screen(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut sounds: MessageWriter<PlaySound>,
) {
    sounds.write(PlaySound("victory"));
    commands.spawn((
        Sprite::from_image(assets.victory.clone()),
        Transform::from_xyz(0.0, 0.0, Z_OVERLAY),
        DespawnOnExit(AppState::Victory),
    ));
}
