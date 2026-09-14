#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Speed {
    Fast,
    #[default]
    Normal,
    Slow,
}

impl Speed {
    pub const fn pose(self) -> u32 {
        match self {
            Self::Fast => 2,
            Self::Normal => 3,
            Self::Slow => 4,
        }
    }

    pub const fn crumble(self) -> u32 {
        match self {
            Self::Fast => 2,
            Self::Normal => 4,
            Self::Slow => 5,
        }
    }

    pub const fn walk(self) -> u32 {
        self.pose() - 1
    }

    pub const fn fall(self, continued: bool) -> u32 {
        match self {
            Self::Fast => 2,
            Self::Normal if continued => 2,
            Self::Normal => 3,
            Self::Slow => 4,
        }
    }
}
