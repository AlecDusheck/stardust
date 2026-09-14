//! Importer for the original game's data: `TEXT` resources 201–250 hold one
//! 16×12 grid each, one character per cell, rows separated by `\r`.

use crate::{Hero, Level, ParseError, Tile};

pub const LEVEL_COUNT: usize = 50;
/// Resource id of the first level's `TEXT` resource.
pub const FIRST_RESOURCE_ID: i16 = 201;

/// Character codes documented by the author in `TEXT` 111, plus the
/// undocumented ones found in the level data.
pub fn tile_from_code(c: char) -> Option<Tile> {
    Some(match c {
        '0' => Tile::Empty,
        '1' => Tile::StarWall,
        '2' => Tile::GrayWall,
        '3' => Tile::Entrance,
        '4' => Tile::Exit,
        '5' => Tile::WarpPocket,
        '6' => Tile::HotWall,
        '7' => Tile::Star,
        '8' => Tile::FallWall,
        '!' => Tile::OneWayLeft,
        '@' => Tile::OneWayRight,
        '#' => Tile::Elevator,
        '$' => Tile::Tunnel,
        '%' => Tile::PhantomWall,
        '(' => Tile::VictoryHero,
        ')' => Tile::VictoryMark,
        '/' => Tile::Beam,
        '^' => Tile::Chevron,
        '&' => Tile::Rune,
        _ => return None,
    })
}

/// Parse one original level grid. `number` is 1-based and picks the name,
/// password and hero.
pub fn parse_level(number: usize, text: &str) -> Result<Level, ParseError> {
    let rows: Vec<String> = text
        .split(['\r', '\n'])
        .filter(|r| !r.is_empty())
        .map(|r| {
            r.chars()
                .map(|c| {
                    tile_from_code(c)
                        .map(crate::legend::to_char)
                        .ok_or(ParseError::UnknownChar(c))
                })
                .collect::<Result<String, _>>()
        })
        .collect::<Result<_, _>>()?;
    let mut level = Level::from_rows(level_name(number), &rows)?;
    level.password = password(number).map(str::to_owned);
    // The second hero takes over after level 25 ("something really cool happens").
    level.hero = if number > 25 { Hero::B } else { Hero::A };
    Ok(level)
}

pub fn level_name(number: usize) -> String {
    const ONES: [&str; 20] = [
        "Zero",
        "One",
        "Two",
        "Three",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Eleven",
        "Twelve",
        "Thirteen",
        "Fourteen",
        "Fifteen",
        "Sixteen",
        "Seventeen",
        "Eighteen",
        "Nineteen",
    ];
    const TENS: [&str; 6] = ["", "", "Twenty", "Thirty", "Forty", "Fifty"];
    match number {
        0..20 => ONES[number].to_owned(),
        20..60 if number.is_multiple_of(10) => TENS[number / 10].to_owned(),
        20..60 => format!("{}-{}", TENS[number / 10], ONES[number % 10]),
        _ => number.to_string(),
    }
}

/// Passwords for levels 1–49, stored in reverse order in CODE 1.
const PASSWORDS: [&str; LEVEL_COUNT - 1] = [
    "BRINTOJUM",
    "MULUROS",
    "SUMER",
    "TIHLIL",
    "DAILEHL",
    "IRSUDI",
    "VILANIA",
    "SORISI",
    "LIVERIGUS",
    "DEEMA",
    "MALIGOPANY",
    "BALZEBUREB",
    "MINILEF",
    "DAEMONDRA",
    "RAUDAS",
    "KALAVIMAN",
    "NORHDYTE",
    "BAHMBUA",
    "KINDUE",
    "DODI",
    "SAEDALUD",
    "COMALDES",
    "RUSTUN",
    "NARAELE",
    "RAUNTEC",
    "FERULIC",
    "MENDAGEY",
    "LUNTATAS",
    "SIREA",
    "VANULC",
    "CELDRA",
    "BELAB",
    "AIGA",
    "SIIS",
    "YITH",
    "RETAIMA",
    "ULIMI",
    "RONOI",
    "GROI",
    "CERIC",
    "VERNESLECS",
    "PALFYTR",
    "AETUFIBUL",
    "TOMIRAUN",
    "TINTEK",
    "SOBEC",
    "ECUIT",
    "CENENCION",
    "EHLNE",
];

/// Password for a 1-based level number.
pub fn password(number: usize) -> Option<&'static str> {
    PASSWORDS
        .get(49_usize.checked_sub(number)?)
        .copied()
        .filter(|_| number != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pos;

    #[test]
    fn names_match_the_originals() {
        assert_eq!(level_name(1), "One");
        assert_eq!(level_name(25), "Twenty-Five");
        assert_eq!(level_name(40), "Forty");
        assert_eq!(level_name(50), "Fifty");
    }

    #[test]
    fn passwords_decode() {
        assert_eq!(password(1), Some("EHLNE"));
        assert_eq!(password(47), Some("SUMER"));
        assert_eq!(password(50), None);
        assert_eq!(password(51), None);
    }

    #[test]
    fn parses_original_grid() {
        let text = "0000\r0304\r2!@$";
        let level = parse_level(30, text).unwrap();
        assert_eq!((level.width, level.height), (4, 3));
        assert_eq!(level.get(Pos::new(1, 1)), Tile::Entrance);
        assert_eq!(level.get(Pos::new(3, 2)), Tile::Tunnel);
        assert_eq!(level.hero, Hero::B);
        assert_eq!(level.name, "Thirty");
    }
}
