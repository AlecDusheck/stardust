use crate::rules;
use stardust_level::{Level, Pos, Tile};

/// Keys held at an action boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub magic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    pub const fn dx(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hero {
    pub pos: Pos,
    pub facing: Facing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    FellOffScreen,
    WarpPocket,
    HotWall,
}

/// Observable changes from one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Turned(Facing),
    Walked { from: Pos, to: Pos },
    Fell { from: Pos, to: Pos },
    Rose { from: Pos, to: Pos },
    Landed(Pos),
    Levitated { from: Pos, to: Pos },
    Created { pos: Pos, tile: Tile },
    Destroyed { pos: Pos, tile: Tile },
    Crumbled(Pos),
    Crumbling(Pos),
    Warped { from: Pos, to: Pos },
    MagicDud,
    Died(Cause),
    Respawned(Pos),
    LevelComplete,
    GameWon,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    Stand,
    Walk,
    LookUp,
    Crouch,
    HoldCrouch,
    Uncrouch,
    Fall,
    Rise,
    MagicSide,
    MagicDown,
    MagicUp,
    Dissolve,
    Respawn,
    Warp,
    Exit,
    Win,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Stance {
    #[default]
    Standing,
    LookingUp,
    Crouching,
    Falling,
    Respawning,
}

#[derive(Debug, Clone)]
struct Crumble {
    pos: Pos,
    frame: u8,
    deadline: u64,
}

#[derive(Debug, Clone)]
pub struct Sim {
    level: Level,
    spawn: Pos,
    hero: Hero,
    stance: Stance,
    action: Action,
    warped: bool,
    turn_time: u64,
    clock: u64,
    start: u64,
    deadline: u64,
    crumbling: Vec<Crumble>,
    finished: bool,
    pub speed: crate::Speed,
    frames: Vec<crate::Frame>,
}

impl Sim {
    pub fn new(level: &Level) -> Self {
        let spawn = level
            .tiles()
            .filter(|(_, tile)| *tile == Tile::Entrance)
            .map(|(pos, _)| pos)
            .max_by_key(|pos| (pos.x, pos.y))
            .unwrap_or(Pos::new(0, 0));
        Self {
            level: level.clone(),
            spawn,
            hero: Hero {
                pos: spawn,
                facing: Facing::Right,
            },
            stance: Stance::Standing,
            action: Action::Stand,
            warped: false,
            turn_time: 0,
            clock: 100,
            start: 100,
            deadline: 0,
            crumbling: Vec::new(),
            finished: false,
            speed: crate::Speed::default(),
            frames: Vec::new(),
        }
    }

    /// Initial materialization uses the same scheduler and poses as respawning.
    pub fn enter(&mut self) {
        self.stance = Stance::Respawning;
        self.step(Input::default(), 0);
        self.frames[0].sounds = vec![204];
    }

    pub const fn level(&self) -> &Level {
        &self.level
    }
    pub const fn hero(&self) -> &Hero {
        &self.hero
    }
    pub const fn finished(&self) -> bool {
        self.finished
    }
    pub fn set_facing(&mut self, facing: Facing) {
        self.hero.facing = facing;
    }
    pub const fn action(&self) -> Action {
        self.action
    }
    pub const fn stance(&self) -> Stance {
        self.stance
    }
    pub fn frames(&self) -> &[crate::Frame] {
        &self.frames
    }
    pub fn elapsed_ticks(&self) -> u64 {
        self.clock - self.start
    }

    fn tile(&self, pos: Pos) -> Tile {
        if pos.x < 0 || pos.x >= self.level.width as i32 || pos.y < 0 {
            Tile::GrayWall
        } else if pos.y >= self.level.height as i32 {
            Tile::Empty
        } else {
            self.level.get(pos)
        }
    }
    fn below(&self) -> Pos {
        self.hero.pos.offset(0, 1)
    }
    fn supported(&self) -> bool {
        rules::supports(self.tile(self.below()))
    }

    /// Sample held keys at an action boundary. Ticks are elapsed since the previous sample.
    pub fn step(&mut self, input: Input, ticks: u32) -> Vec<Event> {
        let mut events = Vec::new();
        if self.finished {
            self.frames.clear();
            return events;
        }
        self.clock = self.clock.max(self.start + u64::from(ticks));
        self.start = self.clock;
        let previous = self.stance;
        self.action = self.choose_action(input, &mut events);
        if previous == Stance::Falling
            && !matches!(self.action, Action::Fall | Action::Rise | Action::Dissolve)
        {
            events.push(Event::Landed(self.hero.pos));
        }
        let before = self.hero;
        let level = self.level.clone();
        self.execute(&mut events);
        self.frames = crate::animate(
            self.action,
            previous,
            before,
            self.hero,
            &level,
            self.speed,
            &events,
        );
        if self.action == Action::Respawn {
            self.clock += 30;
        }
        for index in 0..self.frames.len() {
            let delay = self.frames[index].ticks;
            self.clock = self.clock.max(self.deadline);
            self.deadline = self.clock + u64::from(delay);
            let draws = self.tick_crumbling(&mut events);
            self.frames[index].after.extend(draws);
        }
        let below = self.below();
        if self.stance != Stance::Respawning
            && self.tile(below) == Tile::FallWall
            && !self.crumbling.iter().any(|c| c.pos == below)
        {
            self.crumbling.push(Crumble {
                pos: below,
                frame: 0,
                deadline: self.clock + u64::from(self.speed.crumble()),
            });
            events.push(Event::Crumbling(below));
            if let Some(frame) = self.frames.last_mut() {
                frame.after_sounds.push(210);
            }
        }
        let draws = self.tick_crumbling(&mut events);
        if let Some(frame) = self.frames.last_mut() {
            frame.after.extend(draws);
            if events
                .iter()
                .any(|e| matches!(e, Event::Died(Cause::FellOffScreen)))
            {
                frame.after_sounds.push(110);
            }
        }
        events
    }

    fn choose_action(&mut self, input: Input, events: &mut Vec<Event>) -> Action {
        if self.stance == Stance::Respawning {
            return Action::Respawn;
        }
        let mut action = match self.stance {
            Stance::Crouching => {
                if input.down && input.magic {
                    Action::MagicDown
                } else if input.up || !input.down {
                    Action::Uncrouch
                } else {
                    Action::HoldCrouch
                }
            }
            Stance::LookingUp => {
                if input.left {
                    self.hero.facing = Facing::Left;
                }
                if input.right {
                    self.hero.facing = Facing::Right;
                }
                if self.tile(self.hero.pos) == Tile::Exit {
                    Action::Exit
                } else if input.up && input.magic {
                    Action::MagicUp
                } else if input.up {
                    Action::LookUp
                } else {
                    Action::Stand
                }
            }
            _ => {
                let mut walk = false;
                for (pressed, facing) in [(input.left, Facing::Left), (input.right, Facing::Right)]
                {
                    if !pressed {
                        continue;
                    }
                    if self.hero.facing != facing {
                        self.hero.facing = facing;
                        self.turn_time = self.clock;
                        events.push(Event::Turned(facing));
                    } else if self.clock > self.turn_time + 2 {
                        walk = true;
                    }
                }
                if input.up {
                    Action::LookUp
                } else if input.down {
                    Action::Crouch
                } else if input.magic {
                    Action::MagicSide
                } else if walk {
                    Action::Walk
                } else {
                    Action::Stand
                }
            }
        };
        let here = self.tile(self.hero.pos);
        if !self.supported() && (here != Tile::Tunnel || self.stance == Stance::Falling) {
            action = Action::Fall;
        }
        if here == Tile::Elevator
            || (self.stance == Stance::LookingUp
                && self.tile(self.hero.pos.offset(0, -1)) == Tile::Elevator)
        {
            action = Action::Rise;
        }
        if here == Tile::WarpPocket || self.tile(self.below()) == Tile::HotWall {
            return Action::Dissolve;
        }
        if here == Tile::PhantomWall && !self.warped {
            return Action::Warp;
        }
        if matches!(self.stance, Stance::Standing | Stance::Falling)
            && rules::wins_game(self.tile(self.hero.pos.offset(-1, 0)))
        {
            return Action::Win;
        }
        action
    }

    fn execute(&mut self, events: &mut Vec<Event>) {
        match self.action {
            Action::Stand => self.stance = Stance::Standing,
            Action::LookUp => self.stance = Stance::LookingUp,
            Action::Crouch | Action::HoldCrouch => self.stance = Stance::Crouching,
            Action::Uncrouch => self.stance = Stance::Standing,
            Action::Walk => {
                self.warped = false;
                let from = self.hero.pos;
                let to = self.front();
                if rules::enterable_sideways(self.tile(to), self.hero.facing) {
                    self.hero.pos = to;
                    events.push(Event::Walked { from, to });
                }
                self.stance = Stance::Standing;
            }
            Action::Fall => {
                self.warped = false;
                let from = self.hero.pos;
                self.hero.pos = self.below();
                events.push(Event::Fell {
                    from,
                    to: self.hero.pos,
                });
                self.stance = Stance::Falling;
                if self.hero.pos.y >= self.level.height as i32 {
                    self.die(Cause::FellOffScreen, events);
                }
            }
            Action::Rise => {
                let from = self.hero.pos;
                let to = from.offset(0, -1);
                if rules::enterable_from_below(self.tile(to)) {
                    self.warped = false;
                    self.hero.pos = to;
                    events.push(Event::Rose { from, to });
                } else {
                    self.destroy(from, events);
                }
                self.stance = Stance::Falling;
            }
            Action::MagicSide => {
                self.magic_side(events);
                self.stance = Stance::Standing;
            }
            Action::MagicDown => {
                self.magic_down(events);
                self.stance = Stance::Crouching;
            }
            Action::MagicUp => {
                self.magic_up(events);
                self.stance = Stance::LookingUp;
            }
            Action::Dissolve => self.die(
                if self.tile(self.hero.pos) == Tile::WarpPocket {
                    Cause::WarpPocket
                } else {
                    Cause::HotWall
                },
                events,
            ),
            Action::Respawn => {
                self.hero.pos = self.spawn;
                self.stance = Stance::Standing;
                self.warped = false;
                events.push(Event::Respawned(self.spawn));
            }
            Action::Warp => {
                let from = self.hero.pos;
                for dy in 1..=self.level.height as i32 {
                    let to = Pos::new(from.x, (from.y + dy) % self.level.height as i32);
                    if self.tile(to) == Tile::PhantomWall {
                        self.hero.pos = to;
                        events.push(Event::Warped { from, to });
                        break;
                    }
                }
                self.stance = Stance::Standing;
                self.warped = true;
            }
            Action::Exit => {
                self.finished = true;
                events.push(Event::LevelComplete);
            }
            Action::Win => {
                self.hero.facing = Facing::Left;
                self.finished = true;
                events.push(Event::GameWon);
            }
        }
    }

    fn tick_crumbling(&mut self, events: &mut Vec<Event>) -> Vec<crate::Draw> {
        let mut draws = Vec::new();
        for crumble in &mut self.crumbling {
            if self.clock < crumble.deadline {
                continue;
            }
            crumble.frame += 1;
            crumble.deadline = self.clock + u64::from(self.speed.crumble());
            if crumble.frame == 4 {
                self.level.set(crumble.pos, Tile::Empty);
                events.push(Event::Crumbled(crumble.pos));
                draws.push(crate::Draw::Restore {
                    pos: crumble.pos,
                    tile: Tile::Empty,
                });
            } else {
                draws.push(crate::Draw::Blit {
                    sheet: crate::Sheet::Tiles,
                    source: crate::Rect::new(160, (i32::from(crumble.frame) + 6) * 40, 40, 40),
                    destination: crate::Rect::cell(crumble.pos),
                });
            }
        }
        self.crumbling.retain(|c| c.frame < 4);
        draws
    }

    fn front(&self) -> Pos {
        self.hero.pos.offset(self.hero.facing.dx(), 0)
    }

    fn magic_side(&mut self, events: &mut Vec<Event>) {
        let target = self.front();
        let tile = self.tile(target);
        if rules::destructible_sideways(tile) {
            self.destroy(target, events);
        } else if self.level.in_bounds(target)
            && rules::creatable(tile, self.tile(target.offset(0, 1)))
        {
            self.create(target, Tile::BlueBlock, events);
        } else {
            events.push(Event::MagicDud);
        }
    }

    fn magic_down(&mut self, events: &mut Vec<Event>) {
        // Standing on a green block: the block underfoot goes, not the diagonal.
        if self.tile(self.below()) == Tile::GreenBlock {
            self.destroy(self.below(), events);
            return;
        }
        let target = self.front().offset(0, 1);
        let tile = self.tile(target);
        if rules::destructible_diagonally(tile) {
            self.destroy(target, events);
        } else if self.level.in_bounds(target)
            && rules::creatable(tile, self.tile(target.offset(0, 1)))
        {
            self.create(target, Tile::BlueBlock, events);
        } else {
            events.push(Event::MagicDud);
        }
    }

    fn magic_up(&mut self, events: &mut Vec<Event>) {
        let above = self.hero.pos.offset(0, -1);
        if self.tile(above) == Tile::GreenBlock {
            self.destroy(above, events);
            return;
        }
        let here = self.hero.pos;
        let can_lift = self.level.in_bounds(above)
            && rules::creatable(self.tile(here), self.tile(self.below()))
            && self.tile(self.below()) != Tile::GreenBlock
            && rules::enterable_from_below(self.tile(above))
            && self.tile(above) != Tile::Elevator;
        if can_lift {
            self.hero.pos = above;
            events.push(Event::Levitated {
                from: here,
                to: above,
            });
            self.create(here, Tile::GreenBlock, events);
        } else {
            events.push(Event::MagicDud);
        }
    }

    fn create(&mut self, pos: Pos, tile: Tile, events: &mut Vec<Event>) {
        self.level.set(pos, tile);
        events.push(Event::Created { pos, tile });
    }

    fn destroy(&mut self, pos: Pos, events: &mut Vec<Event>) {
        let tile = self.tile(pos);
        self.level.set(pos, Tile::Empty);
        events.push(Event::Destroyed { pos, tile });
    }

    fn die(&mut self, cause: Cause, events: &mut Vec<Event>) {
        events.push(Event::Died(cause));
        self.stance = Stance::Respawning;
        self.warped = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_uses_the_last_entrance_in_column_order() {
        let level = Level::from_rows("test", &["..I.", "I...", "..I.", "####"]).unwrap();
        let mut sim = Sim::new(&level);
        sim.enter();
        assert_eq!(sim.hero().pos, Pos::new(2, 2));
    }

    #[test]
    fn death_preserves_edits_and_restart_restores_the_level() {
        let level = Level::from_rows("test", &["I..", "###"]).unwrap();
        let mut sim = Sim::new(&level);
        sim.step(
            Input {
                magic: true,
                ..Input::default()
            },
            0,
        );
        sim.stance = Stance::Respawning;
        sim.step(Input::default(), 0);
        assert_eq!(sim.level.get(Pos::new(1, 0)), Tile::BlueBlock);
        assert_eq!(Sim::new(&level).level.get(Pos::new(1, 0)), Tile::Empty);
    }

    #[test]
    fn held_up_requires_a_pose_before_levitation() {
        let level = Level::from_rows("test", &["...", "I..", "###"]).unwrap();
        let mut sim = Sim::new(&level);
        sim.hero.pos = Pos::new(1, 1);
        let keys = Input {
            up: true,
            magic: true,
            ..Input::default()
        };
        sim.step(keys, 0);
        assert_eq!(sim.action(), Action::LookUp);
        assert_eq!(sim.hero.pos, Pos::new(1, 1));
        sim.step(keys, 3);
        assert_eq!(sim.action(), Action::MagicUp);
        assert_eq!(sim.hero.pos, Pos::new(1, 0));
    }
}
