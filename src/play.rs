//! Present the original action frames on a 640×480 playfield.

use crate::{AppState, assets::GameAssets, audio::PlaySound, menu, tileset};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use stardust_core::{Action, Draw, Facing, Frame, Input, Rect, Sheet, Sim, Speed};
use stardust_level::{Hero, Level, Tile};
use std::collections::VecDeque;

#[cfg(test)]
#[path = "play_fidelity_tests.rs"]
mod tests;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Playback;

pub fn plugin(app: &mut App) {
    app.init_resource::<Random>()
        .init_resource::<GameSpeed>()
        .add_systems(OnEnter(AppState::Playing), spawn_level)
        .add_systems(
            Update,
            (hotkeys, playback.in_set(Playback))
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
}

#[derive(Resource, Clone)]
pub struct Pending {
    pub level: Level,
    pub source: Source,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Campaign(usize),
    Editor,
}

// QuickDraw Util.a: Park–Miller state, signed low word (except -32768).
#[derive(Resource)]
struct Random(u32);
impl Default for Random {
    fn default() -> Self {
        Self(1)
    }
}
impl Random {
    fn next(&mut self) -> i16 {
        self.0 = (u64::from(self.0) * 16_807 % 2_147_483_647) as u32;
        let value = self.0 as i16;
        if value == i16::MIN { 0 } else { value }
    }
}

#[derive(Resource, Default)]
struct GameSpeed(Speed);

#[derive(Resource)]
struct Play {
    sim: Sim,
    frames: VecDeque<Frame>,
    prepared: Option<Frame>,
    canvas: Canvas,
    image: Handle<Image>,
    deadline: f64,
    prepare_at: f64,
    last_sample: f64,
    presented_at: f64,
    clock: f64,
    exit_frames: u32,
}

struct Canvas {
    pixels: Vec<u8>,
    variants: [usize; 192],
    set: usize,
}

impl Canvas {
    fn new(hero: Hero) -> Self {
        let mut pixels = vec![0; 640 * 480 * 4];
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        Self {
            pixels,
            variants: [0; 192],
            set: usize::from(hero == Hero::B),
        }
    }

    fn draw(&mut self, draw: &Draw, assets: &GameAssets, images: &Assets<Image>) {
        match *draw {
            Draw::Restore { pos, tile } => {
                if !(0..16).contains(&pos.x) || !(0..12).contains(&pos.y) {
                    return;
                }
                let art = tileset::tile_art(tile);
                let index = art[self.variants[(pos.y * 16 + pos.x) as usize] % art.len()];
                self.blit(
                    images.get(&assets.tiles[self.set]).unwrap(),
                    Rect::new((index % 6) as i32 * 40, (index / 6) as i32 * 40, 40, 40),
                    Rect::cell(pos),
                );
            }
            Draw::Blit {
                sheet,
                source,
                destination,
            } => {
                let handle = match sheet {
                    Sheet::Hero => &assets.heroes[self.set],
                    Sheet::Tiles => &assets.tiles[self.set],
                    Sheet::Companions => &assets.companions,
                };
                self.blit(images.get(handle).unwrap(), source, destination);
            }
        }
    }

    fn exit_wipe(&mut self, pos: stardust_level::Pos, frame: u32) {
        let rect = crate::transition::exit(pos, frame);
        let (left, top) = (rect.x, rect.y);
        let (right, bottom) = (left + rect.width, top + rect.height);
        for y in top.max(0)..bottom.min(480) {
            for x in left.max(0)..right.min(640) {
                let i = (y * 640 + x) as usize * 4;
                self.pixels[i..i + 3].fill(0);
            }
        }
    }

    fn reunion_wipe(&mut self, pos: stardust_level::Pos, frame: u32) {
        let rect = crate::transition::reunion(pos, frame);
        let spans = crate::transition::oval_spans(rect);
        for y in rect.y.max(0)..(rect.y + rect.height).min(480) {
            for x in rect.x.max(0)..(rect.x + rect.width).min(640) {
                // The pair is excluded from the original QuickDraw clip region.
                if ((pos.x - 1) * 40..(pos.x + 1) * 40).contains(&x)
                    && (pos.y * 40..(pos.y + 1) * 40).contains(&y)
                {
                    continue;
                }
                let i = (y * 640 + x) as usize * 4;
                let pixel = &mut self.pixels[i..i + 3];
                if frame <= 54 {
                    pixel.copy_from_slice(&[!pixel[0], !pixel[1], !pixel[2]]);
                }
                if frame > 54 || spans[(y - rect.y) as usize].contains(&x) {
                    pixel.fill(0);
                }
            }
        }
    }

    fn blit(&mut self, image: &Image, source: Rect, destination: Rect) {
        let data = image.data.as_ref().unwrap();
        for y in 0..destination.height {
            for x in 0..destination.width {
                let (dx, dy) = (destination.x + x, destination.y + y);
                let (sx, sy) = (
                    source.x + x * source.width / destination.width,
                    source.y + y * source.height / destination.height,
                );
                if !(0..640).contains(&dx)
                    || !(0..480).contains(&dy)
                    || sx < 0
                    || sy < 0
                    || sx >= image.width() as i32
                    || sy >= image.height() as i32
                {
                    continue;
                }
                let src = (sy as usize * image.width() as usize + sx as usize) * 4;
                if data[src + 3] == 0 {
                    continue;
                }
                let dst = (dy as usize * 640 + dx as usize) * 4;
                self.pixels[dst..dst + 4].copy_from_slice(&data[src..src + 4]);
            }
        }
    }
}

fn spawn_level(
    mut commands: Commands,
    pending: Res<Pending>,
    speed: Res<GameSpeed>,
    assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut random: ResMut<Random>,
) {
    let mut sim = Sim::new(&pending.level);
    sim.speed = speed.0;
    let mut canvas = Canvas::new(pending.level.hero);
    for (pos, tile) in pending.level.tiles() {
        if matches!(tile, Tile::StarWall | Tile::GrayWall) {
            canvas.variants[(pos.y * 16 + pos.x) as usize] =
                usize::from((random.next() % 9).unsigned_abs());
        }
        canvas.draw(&Draw::Restore { pos, tile }, &assets, &images);
    }
    sim.set_facing(if random.next() % 2 == 0 {
        Facing::Left
    } else {
        Facing::Right
    });
    let image = images.add(Image::new(
        Extent3d {
            width: 640,
            height: 480,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        canvas.pixels.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands.spawn((
        Sprite::from_image(image.clone()),
        Transform::default(),
        DespawnOnExit(AppState::Playing),
    ));
    sim.enter();
    let frames = sim.frames().to_vec();
    commands.insert_resource(Play {
        sim,
        frames: frames.into(),
        prepared: None,
        canvas,
        image,
        deadline: 30.0,
        prepare_at: 30.0,
        last_sample: 0.0,
        presented_at: 0.0,
        clock: 0.0,
        exit_frames: 0,
    });
}

fn input(keys: &ButtonInput<KeyCode>) -> Input {
    let any = |codes: &[KeyCode]| codes.iter().any(|key| keys.pressed(*key));
    Input {
        left: any(&[KeyCode::ArrowLeft, KeyCode::KeyA]),
        right: any(&[KeyCode::ArrowRight, KeyCode::KeyD]),
        up: any(&[KeyCode::ArrowUp, KeyCode::KeyW]),
        down: any(&[KeyCode::ArrowDown, KeyCode::KeyS]),
        magic: any(&[KeyCode::Space, KeyCode::Enter]),
    }
}

#[allow(clippy::too_many_arguments)]
fn playback(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    speed: Res<GameSpeed>,
    assets: Res<GameAssets>,
    audio: Res<Assets<AudioSource>>,
    mut images: ResMut<Assets<Image>>,
    mut play: ResMut<Play>,
    mut sounds: MessageWriter<PlaySound>,
    mut pending: ResMut<Pending>,
    mut progress: ResMut<menu::Progress>,
    levels: Res<Assets<crate::assets::LevelAsset>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let play = &mut *play;
    play.clock += time.delta_secs_f64() * 60.0;
    // Catch up complete presentation deadlines without sampling input between animation frames.
    for _ in 0..256 {
        if play.prepared.is_none() {
            if play.clock < play.prepare_at {
                break;
            }
            if let Some(frame) = play.frames.pop_front() {
                for draw in &frame.draws {
                    play.canvas.draw(draw, &assets, &images);
                }
                sounds.write_batch(
                    frame
                        .sounds
                        .iter()
                        .filter_map(|id| sound_name(*id))
                        .map(PlaySound),
                );
                play.prepared = Some(frame);
            } else if play.sim.finished() {
                if play.exit_frames == 0 {
                    play.deadline = play.presented_at;
                }
                if play.clock < play.deadline {
                    break;
                }
                if play.sim.action() == Action::Exit && play.exit_frames < 100 {
                    play.exit_frames += 1;
                    play.canvas.exit_wipe(play.sim.hero().pos, play.exit_frames);
                    images.get_mut(&play.image).unwrap().data = Some(play.canvas.pixels.clone());
                    play.deadline += f64::from(play.exit_frames < 100);
                    continue;
                }
                if play.sim.action() == Action::Win && play.exit_frames < 104 {
                    play.exit_frames += 1;
                    play.canvas
                        .reunion_wipe(play.sim.hero().pos, play.exit_frames);
                    images.get_mut(&play.image).unwrap().data = Some(play.canvas.pixels.clone());
                    // The original has no TickCount wait in this wipe.
                    break;
                }
                finish_level(
                    &mut pending,
                    &mut progress,
                    &assets,
                    &levels,
                    &mut next,
                    play.sim.action() == Action::Win,
                );
                break;
            } else {
                play.sim.speed = speed.0;
                play.sim.step(
                    input(&keys),
                    (play.presented_at - play.last_sample).max(0.0) as u32,
                );
                play.last_sample = play.presented_at;
                play.frames = play.sim.frames().to_vec().into();
                if play.sim.action() == Action::Respawn {
                    play.prepare_at = play.presented_at + 30.0;
                    play.deadline = play.deadline.max(play.prepare_at);
                }
                continue;
            }
        }
        if play.clock < play.deadline {
            break;
        }
        let frame = play.prepared.take().unwrap();
        for draw in &frame.after {
            play.canvas.draw(draw, &assets, &images);
        }
        for id in &frame.after_sounds {
            if let Some(name) = sound_name(*id) {
                sounds.write(PlaySound(name));
            }
        }
        images.get_mut(&play.image).unwrap().data = Some(play.canvas.pixels.clone());
        play.presented_at = play.deadline;
        play.deadline += f64::from(frame.ticks);
        if frame.after_sounds.contains(&110) {
            // SndPlay is synchronous for falling death (CODE 1 $40e).
            use bevy::audio::Source as _;
            if let Some(source) = assets
                .sound("death_by_falling")
                .and_then(|handle| audio.get(&handle))
            {
                let duration = source.decoder().total_duration().unwrap_or_default();
                play.presented_at += (duration.as_secs_f64() * 60.0).ceil();
                play.prepare_at = play.presented_at;
                play.deadline = play.deadline.max(play.prepare_at);
            }
        }
    }
}

fn finish_level(
    pending: &mut Pending,
    progress: &mut menu::Progress,
    assets: &GameAssets,
    levels: &Assets<crate::assets::LevelAsset>,
    next: &mut NextState<AppState>,
    won: bool,
) {
    if won {
        next.set(AppState::Victory);
        return;
    }
    match pending.source {
        Source::Campaign(index) if index + 1 < assets.levels.len() => {
            if let Some(level) = levels.get(&assets.levels[index + 1]) {
                progress.level = index + 1;
                *pending = Pending {
                    level: level.0.clone(),
                    source: Source::Campaign(index + 1),
                };
                next.set(AppState::LevelIntro);
            }
        }
        Source::Campaign(_) => next.set(AppState::Victory),
        Source::Editor => next.set(AppState::Editor),
    }
}

fn sound_name(id: u16) -> Option<&'static str> {
    Some(match id {
        110 => "death_by_falling",
        201 => "end_of_level_portal",
        202 => "g_bye_elevator",
        203 => "g_bye_warppocket",
        204 => "first_entrance_portal",
        205 => "death_by_warppocket",
        206 => "death_by_the_coals",
        207 => "magic_blue",
        208 => "magic_dud",
        209 => "magic_green",
        210 => "g_bye_fallwall",
        211 => "g_bye_greenwall",
        212 => "land",
        213 => "other_entrance_portal",
        214 => "g_bye_block",
        215 => "warp",
        _ => return None,
    })
}

fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    pending: Res<Pending>,
    mut speed: ResMut<GameSpeed>,
) {
    for (key, value) in [
        (KeyCode::Digit1, Speed::Fast),
        (KeyCode::Digit2, Speed::Normal),
        (KeyCode::Digit3, Speed::Slow),
    ] {
        if keys.just_pressed(key) {
            speed.0 = value;
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        next.set(if pending.source == Source::Editor {
            AppState::Editor
        } else {
            AppState::Title
        });
    }
    if keys.just_pressed(KeyCode::KeyR) {
        next.set(AppState::LevelIntro);
    }
}
