use crate::write;
use macrsrc::fork::{Resource, ResourceFork};
use macrsrc::pict::{self, Image};
use macrsrc::{mac_roman, snd};
use stardust_level::{Campaign, original};
use std::fmt;
use std::path::Path;

pub struct Counts {
    pub images: usize,
    pub sounds: usize,
    pub levels: usize,
    pub skipped: Vec<String>,
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} images, {} sounds, {} levels",
            self.images, self.sounds, self.levels
        )?;
        if !self.skipped.is_empty() {
            write!(f, " (skipped: {})", self.skipped.join(", "))?;
        }
        Ok(())
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Faithful dump of every PICT, sound, level and text resource.
pub fn vendor(fork: &ResourceFork, dir: &Path) -> Result<Counts> {
    let mut counts = Counts {
        images: 0,
        sounds: 0,
        levels: 0,
        skipped: Vec::new(),
    };
    for res in fork.of_kind(b"PICT") {
        match pict::decode(&res.data) {
            Ok(img) => {
                write(
                    &dir.join("pict").join(file_name(res, "png")),
                    &encode_png(&img)?,
                )?;
                counts.images += 1;
            }
            Err(e) => counts.skipped.push(format!("PICT {} ({e})", res.id)),
        }
    }
    for res in fork.of_kind(b"snd ") {
        let sound = snd::decode(&res.data)?;
        write(
            &dir.join("snd").join(file_name(res, "wav")),
            &sound.to_wav(),
        )?;
        counts.sounds += 1;
    }
    for res in fork.of_kind(b"TEXT") {
        let text = mac_roman(&res.data).replace('\r', "\n");
        let sub = if is_level(res) { "levels" } else { "text" };
        write(&dir.join(sub).join(file_name(res, "txt")), text.as_bytes())?;
        counts.levels += usize::from(is_level(res));
    }
    write(&dir.join("README.md"), VENDOR_README.as_bytes())?;
    Ok(counts)
}

/// Sprite sheets with transparency applied, sounds by name, and levels in
/// the game's own format.
pub fn assets(fork: &ResourceFork, dir: &Path) -> Result<Counts> {
    let mut counts = Counts {
        images: 0,
        sounds: 0,
        levels: 0,
        skipped: Vec::new(),
    };
    let art = dir.join("art");
    for sheet in SHEETS {
        let mut img = pict::decode(&resource(fork, *b"PICT", sheet.pict)?.data)?;
        if let Some(mask) = sheet.mask {
            img.apply_mask(&pict::decode(&resource(fork, *b"PICT", mask)?.data)?);
        }
        if let Some((w, h)) = sheet.crop {
            img = img.crop(0, 0, w, h);
        }
        write(&art.join(format!("{}.png", sheet.name)), &encode_png(&img)?)?;
        counts.images += 1;
    }
    for res in fork.of_kind(b"snd ") {
        let sound = snd::decode(&res.data)?;
        write(
            &dir.join("sfx").join(format!(
                "{}.wav",
                snake_case(res.name.as_deref().unwrap_or("unnamed"))
            )),
            &sound.to_wav(),
        )?;
        counts.sounds += 1;
    }
    let mut campaign = Campaign { levels: Vec::new() };
    for number in 1..=original::LEVEL_COUNT {
        let id = original::FIRST_RESOURCE_ID + (number as i16 - 1);
        let level = original::parse_level(number, &mac_roman(&resource(fork, *b"TEXT", id)?.data))?;
        let file = format!("{number:02}.level.ron");
        write(&dir.join("levels").join(&file), level.to_ron()?.as_bytes())?;
        campaign.levels.push(file);
        counts.levels += 1;
    }
    write(
        &dir.join("levels").join("campaign.ron"),
        campaign.to_ron()?.as_bytes(),
    )?;
    Ok(counts)
}

struct Sheet {
    name: &'static str,
    pict: i16,
    mask: Option<i16>,
    /// Sheets are 40×40 cells; the originals carry padding on the right.
    crop: Option<(u32, u32)>,
}

const fn sheet(
    name: &'static str,
    pict: i16,
    mask: Option<i16>,
    crop: Option<(u32, u32)>,
) -> Sheet {
    Sheet {
        name,
        pict,
        mask,
        crop,
    }
}

const SHEETS: &[Sheet] = &[
    sheet("tiles_a", 130, None, Some((240, 440))),
    sheet("tiles_b", 133, None, Some((240, 440))),
    sheet("hero_a", 128, Some(129), Some((480, 360))),
    sheet("hero_b", 131, Some(132), Some((480, 360))),
    sheet("logo", 101, None, None),
    sheet("about", 127, None, None),
    sheet("story", 301, None, None),
    sheet("instructions", 303, None, None),
    sheet("companions", 134, None, None),
    sheet("victory", 305, None, None),
];

fn resource(fork: &ResourceFork, kind: [u8; 4], id: i16) -> Result<&Resource> {
    fork.get(&kind, id)
        .ok_or_else(|| format!("missing resource {} {id}", mac_roman(&kind)).into())
}

fn is_level(res: &Resource) -> bool {
    let first = original::FIRST_RESOURCE_ID;
    (first..first + original::LEVEL_COUNT as i16).contains(&res.id)
}

fn file_name(res: &Resource, ext: &str) -> String {
    match &res.name {
        Some(name) => format!("{}_{}.{ext}", res.id, snake_case(name)),
        None => format!("{}.{ext}", res.id),
    }
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_owned()
}

fn encode_png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&img.rgba)?;
    Ok(out)
}

const VENDOR_README: &str = "# Vendored Stardust data

Extracted from `Stardust 1.1` (©1995 James Burton, freeware) by
`stardust-extract`. Do not edit by hand; re-run the extractor instead.

- `pict/`   every PICT resource, decoded to PNG
- `snd/`    every sound, as 8-bit mono WAV
- `levels/` the fifty levels in the original character format
- `text/`   remaining text resources, including the author's note to
            resource editors that documents the level codes
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_cases_resource_names() {
        assert_eq!(snake_case("G'bye WarpPocket"), "g_bye_warppocket");
        assert_eq!(snake_case("Program's Begun!"), "program_s_begun");
        assert_eq!(snake_case("J-I-M"), "j_i_m");
    }
}
