//! Level data model shared by the game, the editor and the extractor.
//!
//! Levels are stored as RON with one string per row so they stay readable
//! and diffable; see [`legend`] for the characters.

pub mod legend;
pub mod original;
mod tile;

pub use tile::{Hero, Tile};

use serde::{Deserialize, Serialize};
use std::fmt;

/// Grid position, `(0, 0)` is the top-left cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// Playfield size of the original game.
pub const ORIGINAL_WIDTH: u32 = 16;
pub const ORIGINAL_HEIGHT: u32 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub name: String,
    /// Code the player can type to start at this level.
    pub password: Option<String>,
    pub hero: Hero,
    pub width: u32,
    pub height: u32,
    tiles: Vec<Tile>,
}

impl Level {
    pub fn empty(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            password: None,
            hero: Hero::A,
            width,
            height,
            tiles: vec![Tile::Empty; (width * height) as usize],
        }
    }

    pub fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && (p.x as u32) < self.width && (p.y as u32) < self.height
    }

    /// Tiles outside the grid read as [`Tile::Empty`] so falling off the
    /// bottom or walking off the side needs no special cases.
    pub fn get(&self, p: Pos) -> Tile {
        if self.in_bounds(p) {
            self.tiles[(p.y as u32 * self.width + p.x as u32) as usize]
        } else {
            Tile::Empty
        }
    }

    pub fn set(&mut self, p: Pos, tile: Tile) {
        if self.in_bounds(p) {
            self.tiles[(p.y as u32 * self.width + p.x as u32) as usize] = tile;
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = (Pos, Tile)> + '_ {
        self.tiles.iter().enumerate().map(|(i, &t)| {
            (
                Pos::new(
                    (i as u32 % self.width) as i32,
                    (i as u32 / self.width) as i32,
                ),
                t,
            )
        })
    }

    pub fn find(&self, tile: Tile) -> Option<Pos> {
        self.tiles().find_map(|(p, t)| (t == tile).then_some(p))
    }

    pub fn rows(&self) -> Vec<String> {
        self.tiles
            .chunks(self.width as usize)
            .map(|row| row.iter().map(|t| legend::to_char(*t)).collect())
            .collect()
    }

    pub fn from_rows(
        name: impl Into<String>,
        rows: &[impl AsRef<str>],
    ) -> Result<Self, ParseError> {
        let height = rows.len() as u32;
        let width = rows.first().map_or(0, |r| r.as_ref().chars().count()) as u32;
        if width == 0 || height == 0 {
            return Err(ParseError::Empty);
        }
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for (y, row) in rows.iter().enumerate() {
            let row = row.as_ref();
            if row.chars().count() as u32 != width {
                return Err(ParseError::RaggedRow { row: y });
            }
            for c in row.chars() {
                tiles.push(legend::from_char(c).ok_or(ParseError::UnknownChar(c))?);
            }
        }
        Ok(Self {
            name: name.into(),
            password: None,
            hero: Hero::A,
            width,
            height,
            tiles,
        })
    }

    pub fn to_ron(&self) -> Result<String, ParseError> {
        let file = LevelFile {
            name: self.name.clone(),
            password: self.password.clone(),
            hero: self.hero,
            rows: self.rows(),
        };
        ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::new())
            .map_err(|e| ParseError::Ron(e.to_string()))
    }

    pub fn from_ron(text: &str) -> Result<Self, ParseError> {
        let file: LevelFile = ron::from_str(text).map_err(|e| ParseError::Ron(e.to_string()))?;
        let mut level = Self::from_rows(file.name, &file.rows)?;
        level.password = file.password;
        level.hero = file.hero;
        Ok(level)
    }
}

/// On-disk shape of a level.
#[derive(Serialize, Deserialize)]
struct LevelFile {
    name: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    hero: Hero,
    rows: Vec<String>,
}

/// Ordered list of level files making up a campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campaign {
    pub levels: Vec<String>,
}

impl Campaign {
    pub fn to_ron(&self) -> Result<String, ParseError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())
            .map_err(|e| ParseError::Ron(e.to_string()))
    }

    pub fn from_ron(text: &str) -> Result<Self, ParseError> {
        ron::from_str(text).map_err(|e| ParseError::Ron(e.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    RaggedRow { row: usize },
    UnknownChar(char),
    Ron(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "level has no rows"),
            Self::RaggedRow { row } => {
                write!(f, "row {row} has a different width from the first row")
            }
            Self::UnknownChar(c) => write!(f, "unknown tile character {c:?}"),
            Self::Ron(e) => write!(f, "invalid level file: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_ron() {
        let mut level = Level::from_rows("t", &["I.O", "###"]).unwrap();
        level.password = Some("SUMER".into());
        level.hero = Hero::B;
        let back = Level::from_ron(&level.to_ron().unwrap()).unwrap();
        assert_eq!(back, level);
        assert_eq!(back.get(Pos::new(2, 0)), Tile::Exit);
        assert_eq!(back.get(Pos::new(-1, 0)), Tile::Empty);
    }

    #[test]
    fn rejects_ragged_rows() {
        assert_eq!(
            Level::from_rows("t", &["..", "..."]),
            Err(ParseError::RaggedRow { row: 1 })
        );
    }
}
