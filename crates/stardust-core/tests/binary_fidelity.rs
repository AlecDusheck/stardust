//! Observable state sampled from the original CODE 1 gameplay loop.
//! Run with `cargo test -p stardust-core --test binary_fidelity`.

use stardust_core::{Draw, Facing, Input, Level, Sheet, Sim, Tile};

const FIXTURES: &str = include_str!("fixtures/binary_scenarios.tsv");

fn runtime_type(tile: Tile) -> u8 {
    match tile {
        Tile::Empty | Tile::VictoryMark => 0,
        Tile::StarWall => 1,
        Tile::GrayWall => 2,
        Tile::Entrance => 3,
        Tile::Exit => 4,
        Tile::BlueBlock => 5,
        Tile::GreenBlock => 6,
        Tile::WarpPocket => 7,
        Tile::HotWall => 8,
        Tile::Star | Tile::Beam | Tile::Chevron | Tile::Rune => 9,
        Tile::FallWall => 10,
        Tile::OneWayLeft => 11,
        Tile::OneWayRight => 12,
        Tile::Elevator => 13,
        Tile::Tunnel => 14,
        Tile::PhantomWall => 15,
        Tile::VictoryHero => 16,
    }
}

fn blits(draws: &[Draw]) -> String {
    draws
        .iter()
        .filter_map(|draw| {
            if let Draw::Blit {
                sheet,
                source: s,
                destination: d,
            } = draw
            {
                let kind = match sheet {
                    Sheet::Hero => "h",
                    Sheet::Tiles => "t",
                    Sheet::Companions => "c",
                };
                Some(format!(
                    "{kind},{},{},{},{},{},{},{},{}",
                    s.x, s.y, s.width, s.height, d.x, d.y, d.width, d.height
                ))
            } else if let Draw::Restore { pos, tile } = draw {
                ((0..16).contains(&pos.x) && (0..12).contains(&pos.y))
                    .then(|| format!("r,{},{},{}", pos.x, pos.y, runtime_type(*tile)))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn frame_trace(frames: &[stardust_core::Frame]) -> String {
    frames
        .iter()
        .map(|frame| {
            let sounds = frame
                .sounds
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let tail = frame
                .after_sounds
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}@{}@{sounds}@{}@{tail}",
                frame.ticks,
                blits(&frame.draws),
                blits(&frame.after)
            )
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn check(name: &str) {
    let header = format!("@{name}\t");
    let mut lines = FIXTURES
        .lines()
        .skip_while(|line| !line.starts_with(&header));
    let definition: Vec<_> = lines.next().expect("fixture exists")[header.len()..]
        .split('\t')
        .collect();
    let rows: Vec<_> = definition[0].split(',').collect();
    let mut level = Level::from_rows(name, &rows).unwrap();
    level.hero = if definition[2] == "1" {
        stardust_level::Hero::B
    } else {
        stardust_level::Hero::A
    };
    let mut sim = Sim::new(&level);
    sim.speed = match definition[1] {
        "0" => stardust_core::Speed::Fast,
        "1" => stardust_core::Speed::Slow,
        _ => stardust_core::Speed::Normal,
    };
    for (step, line) in lines.take_while(|line| !line.starts_with('@')).enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        let keys = fields[0];
        let input = Input {
            left: keys.contains('L'),
            right: keys.contains('R'),
            up: keys.contains('U'),
            down: keys.contains('D'),
            magic: keys.contains('M'),
        };
        let events = sim.step(input, fields[1].parse().unwrap());
        let delays: Vec<u32> = fields[6]
            .split(',')
            .filter(|v| !v.is_empty())
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(
            sim.frames().iter().map(|f| f.ticks).collect::<Vec<_>>(),
            delays,
            "{name} sample {step}: frame delays"
        );
        let frames = frame_trace(sim.frames());
        assert_eq!(
            frames, fields[7],
            "{name} sample {step}: artwork rectangles and sound cues"
        );
        assert_eq!(
            sim.elapsed_ticks(),
            fields[8].parse::<u64>().unwrap(),
            "{name} sample {step}: elapsed ticks"
        );
        let expected: Vec<i32> = fields[2].split(',').map(|v| v.parse().unwrap()).collect();
        let hero = sim.hero();
        assert_eq!(
            [hero.pos.x, hero.pos.y],
            expected.as_slice(),
            "{name} sample {step} keys={keys}; events={events:?}"
        );
        assert_eq!(
            hero.facing,
            if fields[3] == "R" {
                Facing::Right
            } else {
                Facing::Left
            },
            "{name} sample {step}: facing"
        );
        assert_eq!(
            sim.finished(),
            fields[4] == "true",
            "{name} sample {step}: completion"
        );
        let expected_grid: Vec<_> = fields[5]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        for (((pos, tile), expected), cell) in sim.level().tiles().zip(expected_grid).zip(0..) {
            assert_eq!(
                runtime_type(tile),
                expected,
                "{name} sample {step}: tile at {pos:?} (cell {cell})"
            );
        }
    }
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => { $(#[test] fn $name() { check(stringify!($name)); })* };
}

scenarios!(
    idle,
    walk,
    turn,
    up_holds_position,
    crouch_holds_position,
    face_and_cast,
    blue_create,
    blue_over_hot,
    star_destroy,
    blue_destroy,
    green_destroy,
    warp_pocket_destroy,
    magic_dud,
    elevator_side_magic,
    elevator_diagonal_magic,
    levitate,
    levitate_into_tunnel,
    elevator_blocked,
    elevator_overhead,
    tunnel_holds_hero,
    vertical_warp,
    warp_wraps,
    companion_adjacency,
    companion_right_square,
    fall,
    fall_offscreen,
    hot_wall,
    warp_pocket,
    crumble_idle,
    crumble_during_magic,
    exit,
    decoration_47,
    decoration_34,
    decoration_38,
    sideways_60,
    sideways_62,
    sideways_124,
    sideways_94,
    sideways_37,
);

#[test]
fn tile_interactions_and_input_combinations_at_all_speeds() {
    let mut failures = Vec::new();
    for line in FIXTURES.lines().filter(|line| {
        line.starts_with("@matrix_")
            || line.starts_with("@speed_")
            || line.starts_with("@campaign_")
    }) {
        let name = line[1..].split('\t').next().unwrap();
        if std::panic::catch_unwind(|| check(name)).is_err() {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "mismatches: {}", failures.join(", "));
}
