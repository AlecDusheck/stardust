//! CODE 1 $1e96 and $1f86: exit expansion and reunion wipe.
use stardust_core::{Pos, Rect};

pub fn exit(pos: Pos, frame: u32) -> Rect {
    expand(Rect::cell(pos), frame as i32, 50)
}

pub fn reunion(pos: Pos, frame: u32) -> Rect {
    let origin = Rect::new(pos.x * 40 + 7, pos.y * 40 + 20, 0, 0);
    let (step, divisor) = if frame <= 54 {
        (frame, 36)
    } else {
        (frame - 54, 50)
    };
    expand(origin, step as i32, divisor)
}

fn expand(rect: Rect, n: i32, divisor: i32) -> Rect {
    // THINK Pascal truncates the product to a signed word before division.
    let interpolate = |from: i32, to: i32| {
        from + i32::from(((to - from) as i16).wrapping_mul(n as i16)) / divisor
    };
    let left = interpolate(rect.x, 0);
    let top = interpolate(rect.y, 0);
    let right = interpolate(rect.x + rect.width, 640);
    let bottom = interpolate(rect.y + rect.height, 480);
    Rect::new(left, top, right - left, bottom - top)
}

/// QuickDraw's half-pixel scan conversion (DrawArc.a, InitOval/BumpOval).
pub fn oval_spans(rect: Rect) -> Vec<std::ops::Range<i32>> {
    if rect.width <= 0 || rect.height <= 0 {
        return Vec::new();
    }
    let ratio =
        ((i64::from(rect.height) << 16) + i64::from(rect.width / 2)) / i64::from(rect.width);
    let mut odd = ratio * ratio;
    let bump = 2 * odd;
    let mut square = 0_i64;
    let mut remaining = 2 * i64::from(rect.height) - 1;
    let mut left = (i64::from(rect.x) << 16) + (i64::from(rect.width) << 15);
    let mut right = left + 32768;
    let mut spans = Vec::new();
    for row in 0..rect.height {
        while (square >> 32) < remaining {
            left -= 32768;
            right += 32768;
            square += odd;
            odd += bump;
        }
        while (square >> 32) > remaining {
            left += 32768;
            right -= 32768;
            odd -= bump;
            square -= odd;
        }
        spans.push((left >> 16) as i32..(right >> 16) as i32);
        remaining -= 4 * i64::from(2 - rect.height + 2 * row);
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_rectangles_match_the_original_cpu_trace() {
        let mut reunion_mode = false;
        let mut frame = 0;
        for line in include_str!("../tests/fixtures/transitions.tsv").lines() {
            if line.starts_with('@') {
                reunion_mode = line == "@reunion";
                frame = 0;
                continue;
            }
            let fields: Vec<_> = line.split('\t').collect();
            if fields[0] == "oval" {
                continue;
            }
            frame += 1;
            let rect = if reunion_mode {
                reunion(Pos::new(4, 4), frame)
            } else {
                exit(Pos::new(4, 4), frame)
            };
            let expected: Vec<i32> = fields[1..5].iter().map(|s| s.parse().unwrap()).collect();
            assert_eq!(
                [rect.y, rect.x, rect.y + rect.height, rect.x + rect.width],
                expected.as_slice(),
                "{line}"
            );
            if !reunion_mode {
                assert_eq!(fields[5].parse::<u32>().unwrap(), frame - 1);
            }
        }
        assert_eq!(frame, 104);
    }
    #[test]
    fn quickdraw_oval_scanlines_keep_half_pixel_rounding() {
        assert_eq!(oval_spans(Rect::new(0, 0, 4, 4)), [1..3, 0..4, 0..4, 1..3]);
        assert_eq!(
            oval_spans(Rect::new(10, 20, 5, 5)),
            [11..14, 10..15, 10..15, 10..15, 11..14]
        );
        assert!(oval_spans(Rect::new(0, 0, -1, 4)).is_empty());
    }
}
