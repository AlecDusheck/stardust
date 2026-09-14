//! Resting tile artwork. Animation rectangles live in stardust-core.

use stardust_level::Tile;

pub const TILE_COLUMNS: usize = 6;

const fn t(row: usize, col: usize) -> usize {
    row * TILE_COLUMNS + col
}

const EMPTY: [usize; 1] = [t(0, 0)];
const STAR_WALL: [usize; 9] = [
    t(1, 0),
    t(2, 0),
    t(3, 0),
    t(4, 0),
    t(5, 0),
    t(6, 0),
    t(7, 0),
    t(8, 0),
    t(9, 0),
];
const GRAY_WALL: [usize; 9] = [
    t(1, 1),
    t(2, 1),
    t(3, 1),
    t(4, 1),
    t(5, 1),
    t(6, 1),
    t(7, 1),
    t(8, 1),
    t(9, 1),
];
const ENTRANCE: [usize; 1] = [t(9, 3)];
const EXIT: [usize; 1] = [t(10, 3)];
const WARP_POCKET: [usize; 1] = [t(3, 4)];
const HOT_WALL: [usize; 1] = [t(4, 4)];
const STAR: [usize; 1] = [t(5, 4)];
const FALL_WALL: [usize; 1] = [t(6, 4)];
const ELEVATOR: [usize; 1] = [t(2, 5)];
const TUNNEL: [usize; 1] = [t(6, 5)];
const PHANTOM_WALL: [usize; 1] = [t(8, 5)];
const VICTORY_HERO: [usize; 1] = [t(7, 5)];
const BLUE_BLOCK: [usize; 1] = [t(10, 0)];
const GREEN_BLOCK: [usize; 1] = [t(2, 3)];
const DECORATION: [usize; 1] = [t(9, 3)];

/// CODE 1 $3dc2–406e. Wall variants are chosen once when loading a level.
pub const fn tile_art(tile: Tile) -> &'static [usize] {
    match tile {
        Tile::Empty => &EMPTY,
        Tile::StarWall => &STAR_WALL,
        Tile::GrayWall => &GRAY_WALL,
        Tile::Entrance => &ENTRANCE,
        Tile::Exit => &EXIT,
        Tile::WarpPocket => &WARP_POCKET,
        Tile::HotWall => &HOT_WALL,
        Tile::Star => &STAR,
        Tile::FallWall => &FALL_WALL,
        Tile::OneWayLeft => &[5],
        Tile::OneWayRight => &[11],
        Tile::Elevator => &ELEVATOR,
        Tile::Tunnel => &TUNNEL,
        Tile::PhantomWall => &PHANTOM_WALL,
        Tile::VictoryHero => &VICTORY_HERO,
        Tile::VictoryMark => &EMPTY,
        Tile::BlueBlock => &BLUE_BLOCK,
        Tile::GreenBlock => &GREEN_BLOCK,
        Tile::Beam => &DECORATION,
        Tile::Chevron => &[59],
        Tile::Rune => &[65],
    }
}
