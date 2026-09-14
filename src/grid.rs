//! Cell ↔ world-space conversion. The playfield is centred on the origin;
//! cell `(0, 0)` is top-left, as in the level files.

use bevy::prelude::*;
use stardust_level::{ORIGINAL_HEIGHT, ORIGINAL_WIDTH, Pos};

pub const TILE: f32 = 40.0;
pub const PLAYFIELD: Vec2 = Vec2::new(ORIGINAL_WIDTH as f32 * TILE, ORIGINAL_HEIGHT as f32 * TILE);

/// Depth layers, back to front.
pub const Z_TILE: f32 = 0.0;
pub const Z_OVERLAY: f32 = 5.0;

pub fn cell_center(p: Pos) -> Vec2 {
    Vec2::new(
        p.x as f32 * TILE + TILE / 2.0 - PLAYFIELD.x / 2.0,
        PLAYFIELD.y / 2.0 - p.y as f32 * TILE - TILE / 2.0,
    )
}

pub fn cell_at(world: Vec2) -> Option<Pos> {
    let x = ((world.x + PLAYFIELD.x / 2.0) / TILE).floor();
    let y = ((PLAYFIELD.y / 2.0 - world.y) / TILE).floor();
    (x >= 0.0 && y >= 0.0 && x < ORIGINAL_WIDTH as f32 && y < ORIGINAL_HEIGHT as f32)
        .then(|| Pos::new(x as i32, y as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for (x, y) in [(0, 0), (15, 11), (7, 3)] {
            let p = Pos::new(x, y);
            assert_eq!(cell_at(cell_center(p)), Some(p));
        }
        assert_eq!(cell_at(Vec2::new(-1000.0, 0.0)), None);
    }
}
