//! Password representation is deliberately irrelevant; decoded destinations must match.

use stardust_level::{Hero, Level, original};
use std::path::Path;

#[test]
fn passwords_preserve_original_level_destinations() {
    let reference: Vec<_> = include_str!("fixtures/original_passwords.txt")
        .lines()
        .collect();
    assert_eq!(reference.len(), 50);
    let differences: Vec<_> = reference
        .iter()
        .enumerate()
        .filter_map(|(i, expected)| {
            let actual = original::password(i + 1).unwrap_or("");
            (actual != *expected)
                .then(|| format!("level {}: original={expected:?}, port={actual:?}", i + 1))
        })
        .collect();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn all_fifty_shipped_grids_and_hero_choices_match_the_imported_campaign() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for number in 1..=50 {
        let raw = std::fs::read_to_string(root.join(format!("vendor/levels/{}.txt", number + 200)))
            .unwrap();
        let imported = original::parse_level(number, &raw).unwrap();
        let saved =
            std::fs::read_to_string(root.join(format!("assets/levels/{number:02}.level.ron")))
                .unwrap();
        let saved = Level::from_ron(&saved).unwrap();
        assert_eq!(saved.rows(), imported.rows(), "level {number}: grid");
        assert_eq!((saved.width, saved.height), (16, 12));
        assert_eq!(saved.hero, if number <= 25 { Hero::A } else { Hero::B });
    }
}
