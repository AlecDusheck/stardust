use crate::{Action, Event, Facing, Hero, Level, Pos, Speed, Stance, Tile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub const fn cell(pos: Pos) -> Self {
        Self::new(pos.x * 40, pos.y * 40, 40, 40)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    Hero,
    Tiles,
    Companions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Draw {
    Restore {
        pos: Pos,
        tile: Tile,
    },
    Blit {
        sheet: Sheet,
        source: Rect,
        destination: Rect,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub ticks: u32,
    pub draws: Vec<Draw>,
    pub sounds: Vec<u16>,
    pub after: Vec<Draw>,
    pub after_sounds: Vec<u16>,
}

struct Animation {
    level: Level,
    facing: Facing,
    frames: Vec<Frame>,
    pending: Frame,
}

impl Animation {
    fn restore(&mut self, pos: Pos) {
        self.pending.draws.push(Draw::Restore {
            pos,
            tile: self.level.get(pos),
        });
    }
    fn blit(&mut self, sheet: Sheet, source: Rect, destination: Rect) {
        self.pending.draws.push(Draw::Blit {
            sheet,
            source,
            destination,
        });
    }
    fn hero(&mut self, col: i32, row: i32, pos: Pos) {
        let x = if self.facing == Facing::Right {
            col * 40
        } else {
            440 - col * 40
        };
        self.restore(pos);
        self.blit(Sheet::Hero, Rect::new(x, row * 40, 40, 40), Rect::cell(pos));
    }
    fn pose(&mut self, col: i32, rows: impl IntoIterator<Item = i32>, pos: Pos, ticks: u32) {
        for row in rows {
            self.hero(col, row, pos);
            self.present(ticks);
        }
    }
    fn sound(&mut self, id: u16) {
        self.pending.sounds.push(id);
    }
    fn present(&mut self, ticks: u32) {
        self.pending.ticks = ticks;
        self.frames.push(std::mem::take(&mut self.pending));
    }
    fn destroy(&mut self, pos: Pos, tile: Tile, pose: u32, sound: bool) {
        let (col, rows, id) = match tile {
            Tile::StarWall => (2, 1..6, 214),
            Tile::BlueBlock => (2, 6..10, 214),
            Tile::GreenBlock => (3, 3..9, 211),
            Tile::WarpPocket => (4, 10..11, 203),
            _ => return,
        };
        if sound {
            self.sound(id);
        }
        for row in rows {
            self.blit(
                Sheet::Tiles,
                Rect::new(col * 40, row * 40, 40, 40),
                Rect::cell(pos),
            );
            self.present(pose);
        }
        self.level.set(pos, Tile::Empty);
        self.restore(pos);
        self.present(0);
    }
    fn green(&mut self, pos: Pos, ticks: u32) {
        self.sound(209);
        let x = if self.facing == Facing::Right {
            120
        } else {
            160
        };
        for i in 1..=8 {
            self.blit(
                Sheet::Tiles,
                Rect::new(x, i * 5, 40, 80),
                Rect::new(pos.x * 40, (pos.y - 1) * 40, 40, 80),
            );
            self.present(ticks);
        }
        self.level.set(pos, Tile::GreenBlock);
    }

    fn blue(&mut self, pos: Pos, speed: Speed) {
        self.sound(207);
        for i in 1..=16 {
            let left = 80 - i * 5;
            let width = (i * 5).min(40);
            let (sx, sy, dx) = if self.facing == Facing::Right {
                (left, 400, pos.x * 40)
            } else {
                (40 + (i - 8).max(0) * 5, 0, pos.x * 40 + 40 - width)
            };
            self.blit(
                Sheet::Tiles,
                Rect::new(sx, sy, width, 40),
                Rect::new(dx, pos.y * 40, width, 40),
            );
            self.present(if speed == Speed::Slow { 2 } else { 1 });
        }
        self.level.set(pos, Tile::BlueBlock);
        if self.level.get(pos.offset(0, 1)) == Tile::HotWall {
            self.destroy(pos, Tile::BlueBlock, speed.pose(), false);
        }
    }
    fn vertical(&mut self, action: Action, previous: Stance, pos: Pos, to: Pos, speed: Speed) {
        let pose = speed.pose();
        if action == Action::Rise && pos == to {
            self.sound(202);
            for row in 3..6 {
                self.blit(
                    Sheet::Tiles,
                    Rect::new(200, row * 40, 40, 40),
                    Rect::cell(pos),
                );
                let x = if self.facing == Facing::Right {
                    120
                } else {
                    320
                };
                self.blit(Sheet::Hero, Rect::new(x, 280, 40, 40), Rect::cell(pos));
                self.present(pose * 2);
            }
            self.level.set(pos, Tile::Empty);
            self.hero(3, 7, pos);
            self.present(pose * 2);
        } else {
            let rows: &[i32] = if action == Action::Fall {
                &[0, 2, 4, 6]
            } else {
                &[4, 2, 0]
            };
            let top = pos.y.min(to.y) * 40;
            let x = if self.facing == Facing::Right {
                120
            } else {
                320
            };
            for row in rows {
                self.restore(Pos::new(pos.x, pos.y.min(to.y)));
                self.restore(Pos::new(pos.x, pos.y.max(to.y)));
                self.blit(
                    Sheet::Hero,
                    Rect::new(x, row * 40, 40, 80),
                    Rect::new(pos.x * 40, top, 40, 80),
                );
                self.present(if action == Action::Fall {
                    speed.fall(previous == Stance::Falling)
                } else {
                    pose
                });
            }
            if action == Action::Rise {
                self.restore(to);
                self.restore(pos);
                self.blit(Sheet::Hero, Rect::new(x, 280, 40, 40), Rect::cell(to));
                self.present(speed.fall(previous == Stance::Falling));
            }
        }
    }
    fn magic(&mut self, action: Action, pos: Pos, to: Pos, speed: Speed, events: &[Event]) {
        let pose = speed.pose();
        let underfoot =
            action == Action::MagicDown && self.level.get(pos.offset(0, 1)) == Tile::GreenBlock;
        let (col, rows) = match action {
            Action::MagicSide => (2, 0..6),
            Action::MagicDown => (4, 0..4),
            _ => (4, 4..7),
        };
        if underfoot {
            self.sound(211);
            self.pose(2, [8], pos, pose);
        } else {
            self.pose(col, rows.clone(), pos, pose);
        }
        for event in events {
            match *event {
                Event::Created {
                    pos: target,
                    tile: Tile::BlueBlock,
                } => self.blue(target, speed),
                Event::Created {
                    pos: target,
                    tile: Tile::GreenBlock,
                } => self.green(target, pose),
                Event::Destroyed { pos: target, tile } => {
                    self.destroy(target, tile, pose, !underfoot);
                }
                Event::MagicDud => {
                    // Blue over a hot wall still grows before burning away.
                    let target =
                        pos.offset(self.facing.dx(), i32::from(action == Action::MagicDown));
                    if action != Action::MagicUp
                        && self.level.get(target) == Tile::Empty
                        && self.level.get(target.offset(0, 1)) == Tile::HotWall
                    {
                        self.blue(target, speed);
                    } else {
                        self.sound(208);
                    }
                }
                _ => {}
            }
        }
        if !underfoot {
            self.pose(col, rows.rev(), to, pose);
        }
    }
}

pub fn animate(
    action: Action,
    previous: Stance,
    before: Hero,
    after: Hero,
    level: &Level,
    speed: Speed,
    events: &[Event],
) -> Vec<Frame> {
    let mut a = Animation {
        level: level.clone(),
        facing: after.facing,
        frames: Vec::new(),
        pending: Frame::default(),
    };
    let pos = before.pos;
    let pose = speed.pose();
    if action == Action::Warp {
        a.sound(215);
    }
    if events.iter().any(|e| matches!(e, Event::Landed(_))) {
        a.sound(212);
    }
    match action {
        Action::Stand => a.pose(0, [0], pos, pose),
        Action::LookUp => a.pose(4, [7], pos, pose),
        Action::Crouch => a.pose(2, [6, 7], pos, pose),
        Action::HoldCrouch => a.pose(2, [7], pos, pose),
        Action::Uncrouch => a.pose(2, [7, 6], pos, pose),
        Action::Walk => {
            if pos == after.pos {
                a.pose(0, [0, 1, 1, 0], pos, speed.walk());
            } else {
                let left = pos.x.min(after.pos.x) * 40;
                for row in 0..9 {
                    a.restore(pos);
                    a.restore(after.pos);
                    let x = if a.facing == Facing::Right { 0 } else { 400 };
                    a.blit(
                        Sheet::Hero,
                        Rect::new(x, row * 40, 80, 40),
                        Rect::new(left, pos.y * 40, 80, 40),
                    );
                    a.present(speed.walk());
                }
            }
        }
        Action::Fall | Action::Rise => a.vertical(action, previous, pos, after.pos, speed),
        Action::MagicSide | Action::MagicDown | Action::MagicUp => {
            a.magic(action, pos, after.pos, speed, events);
        }
        Action::Dissolve => {
            a.sound(if level.get(pos) == Tile::WarpPocket {
                205
            } else {
                206
            });
            a.pose(5, 0..8, pos, pose);
            a.restore(pos);
            a.present(0);
        }
        Action::Respawn => {
            a.sound(213);
            a.pose(5, (0..8).rev(), after.pos, pose);
        }
        Action::Warp => {
            for row in 0..8 {
                a.hero(5, row, pos);
                a.present(2);
                a.hero(5, 7 - row, after.pos);
                a.present(2);
            }
            a.restore(pos);
            a.present(0);
        }
        Action::Exit => {
            a.pose(4, 4..7, pos, pose);
            a.sound(201);
            for col in 3..7 {
                a.hero(col, 8, pos);
                a.present(pose * 2);
            }
        }
        Action::Win => {
            a.pose(0, [0], pos, pose);
            a.sound(201);
            for row in 0..17 {
                let x = if level.hero == stardust_level::Hero::A {
                    0
                } else {
                    80
                };
                a.blit(
                    Sheet::Companions,
                    Rect::new(x, row * 40, 80, 40),
                    Rect::new((pos.x - 1) * 40, pos.y * 40, 80, 40),
                );
                a.present(match speed {
                    Speed::Fast => 7,
                    Speed::Normal => 8,
                    Speed::Slow => 10,
                });
            }
        }
    }
    a.frames
}
