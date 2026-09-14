//! Tile rules from CODE 1's movement and magic tables.

use crate::sim::Facing;
use stardust_level::Tile;

/// Can the hero stand on top of this tile?
pub const fn supports(tile: Tile) -> bool {
    !enterable_from_above(tile)
}

/// Can the hero fall (or be warped) into this tile from above?
pub const fn enterable_from_above(tile: Tile) -> bool {
    matches!(
        tile,
        Tile::Empty
            | Tile::VictoryMark
            | Tile::Star
            | Tile::Beam
            | Tile::Chevron
            | Tile::Rune
            | Tile::Entrance
            | Tile::Exit
            | Tile::Tunnel
            | Tile::PhantomWall
            | Tile::WarpPocket
    )
}

/// Can the hero rise into this tile (levitation or elevator)?
pub const fn enterable_from_below(tile: Tile) -> bool {
    matches!(
        tile,
        Tile::Empty
            | Tile::VictoryMark
            | Tile::Star
            | Tile::Beam
            | Tile::Chevron
            | Tile::Rune
            | Tile::Tunnel
            | Tile::PhantomWall
            | Tile::Elevator
    )
}

/// Can the hero walk into this tile moving in `dir`?
pub const fn enterable_sideways(tile: Tile, dir: Facing) -> bool {
    match tile {
        Tile::Empty
        | Tile::VictoryMark
        | Tile::Star
        | Tile::Beam
        | Tile::Chevron
        | Tile::Rune
        | Tile::Entrance
        | Tile::Exit
        | Tile::Tunnel
        | Tile::PhantomWall
        | Tile::Elevator => true,
        Tile::OneWayLeft => matches!(dir, Facing::Left),
        Tile::OneWayRight => matches!(dir, Facing::Right),
        _ => false,
    }
}

/// Magic aimed straight sideways removes these.
pub const fn destructible_sideways(tile: Tile) -> bool {
    matches!(
        tile,
        Tile::StarWall | Tile::BlueBlock | Tile::GreenBlock | Tile::WarpPocket
    )
}

/// Magic aimed diagonally down removes these (warp pockets are immune).
pub const fn destructible_diagonally(tile: Tile) -> bool {
    matches!(tile, Tile::StarWall | Tile::BlueBlock | Tile::GreenBlock)
}

/// A block may be created in `cell` unless it is occupied or sits directly
/// on a red wall, which burns anything above it.
pub const fn creatable(cell: Tile, below: Tile) -> bool {
    matches!(cell, Tile::Empty | Tile::VictoryMark) && !matches!(below, Tile::HotWall)
}

/// The companion immediately left of the hero ends the game.
pub const fn wins_game(tile: Tile) -> bool {
    matches!(tile, Tile::VictoryHero)
}
