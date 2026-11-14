//! Keyboard handling: turns key state into game commands each frame.
//! Movement is relative to the camera; the app rotates it into world space.

use std::collections::HashSet;

use godot::classes::Input;
use godot::global::Key;
use godot::prelude::*;

pub enum Command {
    /// Movement keys changed; the app reads `direction()` every frame.
    Move,
    Jump,
    Dodge,
    Block(bool),
    Sprint(bool),
    Gather,
    Potion,
    Tame,
    UseSlot(usize),
    Cast(usize),
    Menu(&'static str),
    ToggleCrafting,
    ToggleInventory,
    ToggleMap,
    NextTarget,
    Escape,
    FocusChat,
}

#[derive(Default)]
pub struct InputState {
    last_dir: (i32, i32),
    down: HashSet<i32>,
    blocking: bool,
    sprinting: bool,
    gather_repeat: f32,
    /// Blocks re-opening chat on the same Enter press that sent a message.
    pub chat_block: f32,
}

pub const SPELL_KEYS: [Key; 6] = [Key::R, Key::F, Key::G, Key::T, Key::Z, Key::X];

impl InputState {
    fn just_pressed(&mut self, input: &Gd<Input>, key: Key) -> bool {
        let code = key.ord();
        if input.is_physical_key_pressed(key) {
            self.down.insert(code)
        } else {
            self.down.remove(&code);
            false
        }
    }

    /// Releases movement (e.g. while typing in chat).
    pub fn release(&mut self) -> Option<Command> {
        self.down.clear();
        if self.last_dir != (0, 0) {
            self.last_dir = (0, 0);
            return Some(Command::Move);
        }
        None
    }

    /// The held movement keys as (right, forward).
    pub fn direction(&self) -> (i32, i32) {
        self.last_dir
    }

    pub fn poll(&mut self, delta: f32) -> Vec<Command> {
        let input = Input::singleton();
        let mut out = Vec::new();
        self.chat_block -= delta;
        self.gather_repeat -= delta;

        if self.just_pressed(&input, Key::ENTER) && self.chat_block <= 0.0 {
            out.extend(self.release());
            out.push(Command::FocusChat);
            return out;
        }
        if self.just_pressed(&input, Key::ESCAPE) {
            out.push(Command::Escape);
        }

        let k = |key: Key| input.is_physical_key_pressed(key) as i32;
        let right = (k(Key::D) | k(Key::RIGHT)) - (k(Key::A) | k(Key::LEFT));
        let fwd = (k(Key::W) | k(Key::UP)) - (k(Key::S) | k(Key::DOWN));
        if (right, fwd) != self.last_dir {
            self.last_dir = (right, fwd);
            out.push(Command::Move);
        }
        // Hold-to keys: block (Q) and sprint (Shift).
        let block = input.is_physical_key_pressed(Key::Q);
        if block != self.blocking {
            self.blocking = block;
            out.push(Command::Block(block));
        }
        let sprint = input.is_physical_key_pressed(Key::SHIFT);
        if sprint != self.sprinting {
            self.sprinting = sprint;
            out.push(Command::Sprint(sprint));
        }
        if input.is_physical_key_pressed(Key::E) && self.gather_repeat <= 0.0 {
            self.gather_repeat = 0.3;
            out.push(Command::Gather);
        }
        for (key, cmd) in [
            (Key::SPACE, Command::Jump),
            (Key::ALT, Command::Dodge),
            (Key::V, Command::Potion),
            (Key::Y, Command::Tame),
            (Key::C, Command::ToggleCrafting),
            (Key::I, Command::ToggleInventory),
            (Key::M, Command::ToggleMap),
            (Key::TAB, Command::NextTarget),
            (Key::K, Command::Menu("char")),
            (Key::B, Command::Menu("spells")),
            (Key::H, Command::Menu("pets")),
            (Key::J, Command::Menu("rep")),
            (Key::L, Command::Menu("journal")),
        ] {
            if self.just_pressed(&input, key) {
                out.push(cmd);
            }
        }
        for (i, key) in SPELL_KEYS.into_iter().enumerate() {
            if self.just_pressed(&input, key) {
                out.push(Command::Cast(i));
            }
        }
        let digits = [
            Key::KEY_1,
            Key::KEY_2,
            Key::KEY_3,
            Key::KEY_4,
            Key::KEY_5,
            Key::KEY_6,
            Key::KEY_7,
            Key::KEY_8,
            Key::KEY_9,
        ];
        for (i, key) in digits.into_iter().enumerate() {
            if self.just_pressed(&input, key) {
                out.push(Command::UseSlot(i));
            }
        }
        out
    }
}
