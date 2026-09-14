//! Stardust: a Bevy port of James Burton's 1995 Macintosh puzzle game.

mod assets;
mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod dev;
mod editor;
mod grid;
mod menu;
mod play;
mod tileset;
mod transition;

use bevy::camera::ScalingMode;
use bevy::prelude::*;

/// Top-level flow. `Loading` waits for every asset before anything shows.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    Title,
    Story,
    Instructions,
    Password,
    LevelIntro,
    Playing,
    Victory,
    Editor,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Stardust".into(),
                        resolution: (960, 720).into(),
                        canvas: Some("#game".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::BLACK))
        .init_state::<AppState>()
        .add_plugins((
            assets::plugin,
            audio::plugin,
            menu::plugin,
            play::plugin,
            editor::plugin,
        ))
        .add_systems(Startup, spawn_camera)
        .add_plugins(dev_plugin)
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
use dev::plugin as dev_plugin;

#[cfg(target_arch = "wasm32")]
fn dev_plugin(_: &mut App) {}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: grid::PLAYFIELD.x,
                min_height: grid::PLAYFIELD.y,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
