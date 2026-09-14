//! Drawing expectations from CODE 1.
use super::*;

fn art(tile: Tile, expected: usize) {
    let actual = tileset::tile_art(tile);
    assert_eq!(
        actual[0], expected,
        "wrong original sheet cell for {tile:?}"
    );
}

macro_rules! tile_tests {
    ($($name:ident: $tile:ident => $index:expr),* $(,)?) => { $(
        #[test] fn $name() { art(Tile::$tile, $index); }
    )* };
}

tile_tests!(
    entrance_art: Entrance => 57, exit_art: Exit => 63,
    fall_wall_art: FallWall => 40, left_wall_art: OneWayLeft => 5,
    right_wall_art: OneWayRight => 11, elevator_art: Elevator => 17,
    tunnel_art: Tunnel => 41, warp_art: PhantomWall => 53,
    beam_art: Beam => 57, chevron_art: Chevron => 59, rune_art: Rune => 65,
);

#[test]
fn input_uses_only_currently_held_keys() {
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Space);
    keys.release(KeyCode::Space);
    keys.press(KeyCode::ArrowUp);
    assert_eq!(
        input(&keys),
        Input {
            up: true,
            ..default()
        }
    );
}

#[test]
fn blit_preserves_wide_sprite_positions_and_transparency() {
    let mut pixels = vec![0; 80 * 40 * 4];
    pixels[39 * 4..40 * 4].copy_from_slice(&[255, 20, 30, 255]);
    pixels[40 * 4..41 * 4].copy_from_slice(&[40, 50, 60, 255]);
    let image = Image::new(
        Extent3d {
            width: 80,
            height: 40,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let mut canvas = Canvas::new(Hero::A);
    canvas.blit(&image, Rect::new(0, 0, 80, 40), Rect::new(10, 20, 80, 40));
    let i = (20 * 640 + 49) * 4;
    assert_eq!(
        &canvas.pixels[i..i + 8],
        &[255, 20, 30, 255, 40, 50, 60, 255]
    );
    assert_eq!(&canvas.pixels[i - 4..i], &[0, 0, 0, 255]);
}

fn test_world(rows: &[&str]) -> World {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    let mut images = Assets::<Image>::default();
    let mut pixels = vec![0; 480 * 720 * 4];
    for (i, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        pixel.copy_from_slice(&[(i % 251) as u8, (i / 480 % 251) as u8, 70, 255]);
    }
    let sheet = images.add(Image::new(
        Extent3d {
            width: 480,
            height: 720,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    world.insert_resource(GameAssets {
        tiles: [sheet.clone(), sheet.clone()],
        heroes: [sheet.clone(), sheet.clone()],
        companions: sheet,
        ..default()
    });
    world.insert_resource(images);
    world.init_resource::<Assets<AudioSource>>();
    world.init_resource::<Time<Real>>();
    world.init_resource::<ButtonInput<KeyCode>>();
    world.init_resource::<bevy::ecs::message::Messages<PlaySound>>();
    world.init_resource::<NextState<AppState>>();
    world.init_resource::<menu::Progress>();
    world.init_resource::<Assets<crate::assets::LevelAsset>>();
    world.init_resource::<GameSpeed>();
    world.init_resource::<Random>();
    world.insert_resource(Pending {
        level: Level::from_rows("test", rows).unwrap(),
        source: Source::Editor,
    });
    world.run_system_once(spawn_level).unwrap();
    world.resource_mut::<Play>().sim.set_facing(Facing::Right);
    world
}

fn advance(world: &mut World, ticks: f64) {
    use bevy::ecs::system::RunSystemOnce;
    world
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_secs_f64(ticks / 60.0));
    world.run_system_once(playback).unwrap();
}

#[test]
fn entrance_waits_thirty_ticks_and_carries_its_last_deadline() {
    let mut world = test_world(&["I...............", "################"]);
    advance(&mut world, 29.0);
    assert!(world.resource::<Play>().prepared.is_none());
    assert!(
        world
            .resource::<bevy::ecs::message::Messages<PlaySound>>()
            .is_empty()
    );
    advance(&mut world, 1.0);
    assert_eq!(world.resource::<Play>().presented_at as u64, 30);
    advance(&mut world, 21.0);
    assert_eq!(world.resource::<Play>().presented_at as u64, 51);
    assert_eq!(world.resource::<Play>().deadline as u64, 54);
    advance(&mut world, 2.0);
    assert_eq!(world.resource::<Play>().presented_at as u64, 51);
    advance(&mut world, 1.0);
    assert_eq!(world.resource::<Play>().presented_at as u64, 54);
}

#[test]
fn playback_has_the_same_result_at_different_display_rates() {
    let mut expected = None;
    for rate in [30, 60, 144] {
        let mut world = test_world(&["I...............", "################"]);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        for _ in 0..rate * 2 {
            advance(&mut world, 60.0 / f64::from(rate));
        }
        // Finish between deadlines, avoiding nanosecond rounding at exactly 120 ticks.
        advance(&mut world, 1.0);
        let play = world.resource::<Play>();
        let image = world.resource::<Assets<Image>>().get(&play.image).unwrap();
        let snapshot = (
            play.sim.hero().pos,
            play.presented_at as u64,
            image.data.clone(),
        );
        if let Some(expected) = &expected {
            assert!(
                &snapshot == expected,
                "different state or pixels at display rate {rate}"
            );
        } else {
            expected = Some(snapshot);
        }
    }
}

#[test]
fn resting_walls_keep_the_same_pixels_across_idle_frames() {
    let mut world = test_world(&["I*#~|bg.........", "################"]);
    advance(&mut world, 60.0);
    let before = world.resource::<Play>().canvas.pixels[40 * 4..280 * 4].to_vec();
    advance(&mut world, 60.0);
    assert_eq!(
        world.resource::<Play>().canvas.pixels[40 * 4..280 * 4],
        before
    );
}

#[test]
fn exit_wipe_reaches_the_entire_playfield() {
    let mut canvas = Canvas::new(Hero::A);
    canvas.pixels.fill(255);
    canvas.exit_wipe(stardust_level::Pos::new(7, 5), 1);
    assert_eq!(canvas.pixels[0], 255);
    canvas.exit_wipe(stardust_level::Pos::new(7, 5), 50);
    assert!(
        canvas
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 0, 255])
    );
}

#[test]
fn falling_death_waits_for_sound_then_the_respawn_pause() {
    let mut world = test_world(&[
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "................",
        "I...............",
    ]);
    let sound = world
        .resource_mut::<Assets<AudioSource>>()
        .add(AudioSource {
            bytes: include_bytes!("../assets/sfx/death_by_falling.wav")
                .as_slice()
                .into(),
        });
    world
        .resource_mut::<GameAssets>()
        .sounds
        .push(("death_by_falling", sound));
    advance(&mut world, 64.0);
    assert_eq!(world.resource::<Play>().sim.action(), Action::Fall);
    advance(&mut world, 75.0);
    assert_eq!(world.resource::<Play>().sim.action(), Action::Fall);
    advance(&mut world, 1.0);
    assert_eq!(world.resource::<Play>().sim.action(), Action::Respawn);
    assert_eq!(world.resource::<Play>().prepare_at as u64, 170);
    advance(&mut world, 30.0);
    assert_eq!(world.resource::<Play>().presented_at as u64, 170);
}

#[test]
fn reunion_wipe_preserves_the_pair_and_blacks_out_the_surroundings() {
    let mut canvas = Canvas::new(Hero::A);
    canvas.pixels.fill(255);
    let pos = stardust_level::Pos::new(7, 5);
    for frame in 1..=104 {
        canvas.reunion_wipe(pos, frame);
    }
    for y in 0..480 {
        for x in 0..640 {
            let i = (y * 640 + x) * 4;
            let expected = if (240..320).contains(&x) && (200..240).contains(&y) {
                255
            } else {
                0
            };
            assert_eq!(canvas.pixels[i], expected, "({x}, {y})");
        }
    }
}
