//! ASCII legend used in level files. Chosen to be mnemonic, not to match the
//! original game's digits (see [`crate::original`] for those).

use crate::Tile;

const LEGEND: [(char, Tile); 21] = [
    ('.', Tile::Empty),
    ('*', Tile::StarWall),
    ('#', Tile::GrayWall),
    ('I', Tile::Entrance),
    ('O', Tile::Exit),
    ('U', Tile::WarpPocket),
    ('R', Tile::HotWall),
    ('+', Tile::Star),
    ('~', Tile::FallWall),
    ('<', Tile::OneWayLeft),
    ('>', Tile::OneWayRight),
    ('^', Tile::Elevator),
    ('|', Tile::Tunnel),
    ('%', Tile::PhantomWall),
    ('(', Tile::VictoryHero),
    (')', Tile::VictoryMark),
    ('b', Tile::BlueBlock),
    ('g', Tile::GreenBlock),
    ('/', Tile::Beam),
    ('"', Tile::Chevron),
    ('&', Tile::Rune),
];

pub fn to_char(tile: Tile) -> char {
    LEGEND
        .iter()
        .find(|(_, t)| *t == tile)
        .map_or('?', |(c, _)| *c)
}

pub fn from_char(c: char) -> Option<Tile> {
    LEGEND.iter().find(|(l, _)| *l == c).map(|(_, t)| *t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_has_a_unique_char() {
        for tile in Tile::ALL {
            assert_eq!(from_char(to_char(tile)), Some(tile), "{tile:?}");
        }
        let mut chars: Vec<char> = LEGEND.iter().map(|(c, _)| *c).collect();
        chars.sort_unstable();
        chars.dedup();
        assert_eq!(chars.len(), LEGEND.len());
    }
}
