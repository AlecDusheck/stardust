//! Exported sheets must preserve the original pixels, positions and masks.

use macrsrc::{fork::ResourceFork, pict};
use std::{io::Cursor, path::Path};

fn check_sheet(picture: i16, mask: Option<i16>, asset: &str, width: u32, height: u32) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bytes = std::fs::read(root.join("archive/Stardust_Mac_EN.sit")).unwrap();
    let archive = stuffit::parse(&bytes).unwrap();
    let entry = archive
        .entries
        .iter()
        .find(|e| &e.finder_type == b"APPL")
        .unwrap();
    let raw = archive
        .read_fork(&bytes, entry.resource_fork.as_ref().unwrap())
        .unwrap();
    let fork = ResourceFork::parse(&raw).unwrap();
    let mut expected = pict::decode(&fork.get(b"PICT", picture).unwrap().data).unwrap();
    if let Some(mask) = mask {
        expected.apply_mask(&pict::decode(&fork.get(b"PICT", mask).unwrap().data).unwrap());
    }
    let expected = expected.crop(0, 0, width, height);
    let png = std::fs::read(root.join(format!("assets/art/{asset}.png"))).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (width, height));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    let actual = pict::Image {
        width: info.width,
        height: info.height,
        rgba: pixels,
    };
    for (index, (actual, expected)) in actual
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.rgba.as_chunks::<4>().0)
        .enumerate()
    {
        assert_eq!(actual, expected, "{asset}: pixel {index}");
    }
}

#[test]
fn hero_a_preserves_sprite_pixels_and_masks() {
    check_sheet(128, Some(129), "hero_a", 480, 360);
}

#[test]
fn hero_b_preserves_sprite_pixels_and_masks() {
    check_sheet(131, Some(132), "hero_b", 480, 360);
}

#[test]
fn tile_sheets_preserve_original_pixels() {
    check_sheet(130, None, "tiles_a", 240, 440);
    check_sheet(133, None, "tiles_b", 240, 440);
}
#[test]
fn companions_preserve_original_pixels() {
    check_sheet(134, None, "companions", 160, 680);
}
