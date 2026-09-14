//! Level editor: paint tiles with the mouse, pick from a palette, save to
//! `assets/levels/` (native only) and test-play with `P`.

use crate::assets::{GameAssets, LevelAsset};
use crate::grid::{self, PLAYFIELD, TILE, Z_OVERLAY, Z_TILE};
use crate::play::{Pending, Source};
use crate::{AppState, tileset};
use bevy::camera::ScalingMode;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use stardust_level::{Level, ORIGINAL_HEIGHT, ORIGINAL_WIDTH, Pos, Tile};

pub fn plugin(app: &mut App) {
    app.init_resource::<Editor>()
        .add_systems(
            OnEnter(AppState::Editor),
            (spawn_editor, |p: Single<&mut Projection>| {
                zoom(p, EDITOR_MARGIN);
            }),
        )
        .add_systems(OnExit(AppState::Editor), |p: Single<&mut Projection>| {
            zoom(p, 0.0);
        })
        .add_systems(
            Update,
            (edit_keys, paint, refresh, draw_gizmos)
                .chain()
                .run_if(in_state(AppState::Editor)),
        );
}

#[derive(Resource)]
pub struct Editor {
    pub level: Level,
    /// Campaign slot this level was opened from, for saving in place.
    pub slot: Option<usize>,
    pub brush: Tile,
    pub status: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            level: Level::empty("Untitled", ORIGINAL_WIDTH, ORIGINAL_HEIGHT),
            slot: None,
            brush: Tile::GrayWall,
            status: String::new(),
        }
    }
}

#[derive(Component)]
struct EditCell(Pos);

#[derive(Component)]
struct PaletteCell(Tile);

#[derive(Component)]
struct StatusText;

const PALETTE_Y: f32 = -PLAYFIELD.y / 2.0 - TILE * 0.75;
/// Extra vertical room for the palette and status bar.
const EDITOR_MARGIN: f32 = TILE * 3.0;

fn zoom(mut projection: Single<&mut Projection>, margin: f32) {
    if let Projection::Orthographic(ortho) = &mut **projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
            min_width: PLAYFIELD.x,
            min_height: PLAYFIELD.y + margin,
        };
    }
}

fn spawn_editor(
    mut commands: Commands,
    assets: Res<GameAssets>,
    pending: Option<Res<Pending>>,
    mut editor: ResMut<Editor>,
) {
    // Coming back from a test run: keep whatever was being edited.
    if let Some(pending) = pending.filter(|p| p.source == Source::Editor) {
        editor.level = pending.level.clone();
    }
    let atlas = |index| {
        Sprite::from_atlas_image(
            assets.tiles[0].clone(),
            TextureAtlas {
                layout: assets.tile_layout.clone(),
                index,
            },
        )
    };
    for (pos, _) in editor.level.tiles() {
        commands.spawn((
            atlas(0),
            Transform::from_translation(grid::cell_center(pos).extend(Z_TILE)),
            EditCell(pos),
            DespawnOnExit(AppState::Editor),
        ));
    }
    let count = Tile::ALL.len() as f32;
    for (i, tile) in Tile::ALL.into_iter().enumerate() {
        let x = (i as f32 - (count - 1.0) / 2.0) * (TILE * 0.75);
        commands.spawn((
            atlas(tileset::tile_art(tile)[0]),
            Transform::from_xyz(x, PALETTE_Y, Z_TILE).with_scale(Vec3::splat(0.7)),
            PaletteCell(tile),
            DespawnOnExit(AppState::Editor),
        ));
    }
    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.7, 0.9, 1.0)),
        TextLayout::justify(Justify::Center),
        Transform::from_xyz(0.0, PLAYFIELD.y / 2.0 + TILE * 0.6, Z_OVERLAY),
        StatusText,
        DespawnOnExit(AppState::Editor),
    ));
    editor.status = "left: paint   right: erase   wheel/[ ]: brush   1-9: load level   P: test   ctrl+S: save   N: new   esc: title".into();
}

fn cursor_world(camera: &Camera, camera_tf: &GlobalTransform, window: &Window) -> Option<Vec2> {
    camera
        .viewport_to_world_2d(camera_tf, window.cursor_position()?)
        .ok()
}

fn paint(
    mut editor: ResMut<Editor>,
    camera: Single<(&Camera, &GlobalTransform)>,
    window: Single<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    palette: Query<(&Transform, &PaletteCell)>,
) {
    let scroll: f32 = wheel.read().map(|w| w.y).sum();
    if scroll.abs() > 0.0 {
        cycle_brush(&mut editor, if scroll > 0.0 { -1 } else { 1 });
    }
    let (camera, camera_tf) = *camera;
    let Some(world) = cursor_world(camera, camera_tf, &window) else {
        return;
    };
    if let Some(pos) = grid::cell_at(world) {
        if mouse.pressed(MouseButton::Left) {
            let brush = editor.brush;
            editor.level.set(pos, brush);
        } else if mouse.pressed(MouseButton::Right) {
            editor.level.set(pos, Tile::Empty);
        }
    } else if mouse.just_pressed(MouseButton::Left) {
        let hit = palette
            .iter()
            .find(|(tf, _)| (tf.translation.truncate() - world).abs().max_element() < TILE * 0.35);
        if let Some((_, PaletteCell(tile))) = hit {
            editor.brush = *tile;
        }
    }
}

fn cycle_brush(editor: &mut Editor, by: i32) {
    let i = Tile::ALL
        .iter()
        .position(|t| *t == editor.brush)
        .unwrap_or(0) as i32;
    editor.brush = Tile::ALL[(i + by).rem_euclid(Tile::ALL.len() as i32) as usize];
}

fn edit_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<Editor>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
    assets: Res<GameAssets>,
    levels: Res<Assets<LevelAsset>>,
) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        cycle_brush(&mut editor, -1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        cycle_brush(&mut editor, 1);
    }
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
    if keys.just_pressed(KeyCode::KeyN) {
        *editor = Editor::default();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        commands.insert_resource(Pending {
            level: editor.level.clone(),
            source: Source::Editor,
        });
        next.set(AppState::LevelIntro);
    }
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        save(&mut editor);
    }
    // Digits load campaign levels 1-9; with shift, 11-19 etc. is overkill, so
    // Page Up/Down step through the whole campaign instead.
    let step = if keys.just_pressed(KeyCode::PageDown) {
        1
    } else if keys.just_pressed(KeyCode::PageUp) {
        -1
    } else {
        0
    };
    let digit = (1..=9u8)
        .find(|d| keys.just_pressed(digit_key(*d)))
        .map(|d| usize::from(d) - 1);
    let slot = digit.or_else(|| {
        (step != 0).then(|| {
            (editor.slot.map_or(0, |s| s as i32) + step).rem_euclid(assets.levels.len() as i32)
                as usize
        })
    });
    if let Some(slot) = slot {
        if let Some(level) = assets.levels.get(slot).and_then(|h| levels.get(h)) {
            editor.level = level.0.clone();
            editor.slot = Some(slot);
            editor.status = format!("loaded level {} ({})", slot + 1, editor.level.name);
        }
    }
}

const fn digit_key(d: u8) -> KeyCode {
    match d {
        1 => KeyCode::Digit1,
        2 => KeyCode::Digit2,
        3 => KeyCode::Digit3,
        4 => KeyCode::Digit4,
        5 => KeyCode::Digit5,
        6 => KeyCode::Digit6,
        7 => KeyCode::Digit7,
        8 => KeyCode::Digit8,
        _ => KeyCode::Digit9,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save(editor: &mut Editor) {
    let file = match editor.slot {
        Some(slot) => format!("{:02}.level.ron", slot + 1),
        None => "custom.level.ron".to_owned(),
    };
    let path = std::path::Path::new("assets/levels").join(&file);
    editor.status = match editor
        .level
        .to_ron()
        .map_err(|e| e.to_string())
        .and_then(|ron| std::fs::write(&path, ron).map_err(|e| e.to_string()))
    {
        Ok(()) => format!("saved {}", path.display()),
        Err(e) => format!("save failed: {e}"),
    };
}

#[cfg(target_arch = "wasm32")]
fn save(editor: &mut Editor) {
    editor.status =
        "saving is not available in the browser; copy the RON below from the console".into();
    if let Ok(ron) = editor.level.to_ron() {
        info!("{ron}");
    }
}

fn refresh(
    editor: Res<Editor>,
    mut cells: Query<(&EditCell, &mut Sprite)>,
    mut status: Single<&mut Text2d, With<StatusText>>,
) {
    for (EditCell(pos), mut sprite) in &mut cells {
        let art = tileset::tile_art(editor.level.get(*pos));
        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = art[0];
        }
    }
    status.0 = format!(
        "{}\nbrush: {}   |   {}",
        editor.status,
        editor.brush.name(),
        editor.level.name
    );
}

fn draw_gizmos(
    mut gizmos: Gizmos,
    editor: Res<Editor>,
    palette: Query<(&Transform, &PaletteCell)>,
) {
    gizmos
        .grid_2d(
            Isometry2d::IDENTITY,
            UVec2::new(ORIGINAL_WIDTH, ORIGINAL_HEIGHT),
            Vec2::splat(TILE),
            Color::srgba(1.0, 1.0, 1.0, 0.12),
        )
        .outer_edges();
    for (tf, PaletteCell(tile)) in &palette {
        if *tile == editor.brush {
            gizmos.rect_2d(
                Isometry2d::from_translation(tf.translation.truncate()),
                Vec2::splat(TILE * 0.75),
                Color::srgb(1.0, 0.9, 0.2),
            );
        }
    }
}
