use serde::{Deserialize, Serialize};

/// Everything a grid cell can hold. Behaviour lives in `stardust-core`; the
/// doc comments describe behavior verified against the original binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tile {
    Empty,
    /// Multicoloured wall; magic destroys it.
    StarWall,
    /// Gray wall; indestructible.
    GrayWall,
    /// Where the hero appears (and reappears after "dying").
    Entrance,
    /// Press Up while inside to finish the level.
    Exit,
    /// Solid from the sides and below, open on top; falling in warps you home.
    WarpPocket,
    /// Red wall; standing on it kills, nothing can be built directly above it.
    HotWall,
    /// Walk-through star; no block can be created in its cell.
    Star,
    /// Crumbling starts on touch and advances with the animation scheduler.
    FallWall,
    /// Original `!`: only passable when moving left. Solid vertically.
    OneWayLeft,
    /// Original `@`: only passable when moving right. Solid vertically.
    OneWayRight,
    /// Original `#`: reverses gravity while inside; acts as a floor from above.
    Elevator,
    /// Original `$`: holds a standing hero; a falling hero passes through.
    Tunnel,
    /// Original `%`: warps to the next matching tile below in the same column.
    PhantomWall,
    /// Original `(`: the lost companion; standing immediately to its right wins.
    VictoryHero,
    /// Original `)`: empty cell in the runtime map.
    VictoryMark,
    /// Player-made wall, same rules as [`Tile::StarWall`].
    BlueBlock,
    /// Player-made lift; fragile, cannot be stacked.
    GreenBlock,
    /// Original `/`, `^`, `&`: pass-through, magic-resistant decorations.
    Beam,
    Chevron,
    Rune,
}

impl Tile {
    pub const ALL: [Self; 21] = [
        Self::Empty,
        Self::StarWall,
        Self::GrayWall,
        Self::Entrance,
        Self::Exit,
        Self::WarpPocket,
        Self::HotWall,
        Self::Star,
        Self::FallWall,
        Self::OneWayLeft,
        Self::OneWayRight,
        Self::Elevator,
        Self::Tunnel,
        Self::PhantomWall,
        Self::VictoryHero,
        Self::VictoryMark,
        Self::BlueBlock,
        Self::GreenBlock,
        Self::Beam,
        Self::Chevron,
        Self::Rune,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::StarWall => "Star wall",
            Self::GrayWall => "Gray wall",
            Self::Entrance => "Entrance portal",
            Self::Exit => "Exit portal",
            Self::WarpPocket => "Warp pocket",
            Self::HotWall => "Red wall",
            Self::Star => "Star",
            Self::FallWall => "Fall wall",
            Self::OneWayLeft => "One-way (left)",
            Self::OneWayRight => "One-way (right)",
            Self::Elevator => "Elevator",
            Self::Tunnel => "Tunnel",
            Self::PhantomWall => "Phantom wall",
            Self::VictoryHero => "Companion",
            Self::VictoryMark => "Companion mark",
            Self::BlueBlock => "Blue block",
            Self::GreenBlock => "Green block",
            Self::Beam => "Beam",
            Self::Chevron => "Chevron",
            Self::Rune => "Rune",
        }
    }
}

/// Which of the two lost heroes the player controls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Hero {
    #[default]
    A,
    B,
}
