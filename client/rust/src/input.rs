//! Keyboard handling: turns key state into game commands each frame.

use std::collections::HashSet;

use godot::classes::Input;
use godot::global::Key;
use godot::prelude::*;

pub enum Command {
    Move(i32, i32),
    Attack,
    Gather,
    Potion,
    UseSlot(usize),
    ToggleCrafting,
    FocusChat,
}

#[derive(Default)]
pub struct InputState {
    last_dir: (i32, i32),
    down: HashSet<i32>,
    attack_repeat: f32,
    gather_repeat: f32,
    /// Blocks re-opening chat on the same Enter press that sent a message.
    pub chat_block: f32,
}

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
            return Some(Command::Move(0, 0));
        }
        None
    }

    pub fn poll(&mut self, delta: f32) -> Vec<Command> {
        let input = Input::singleton();
        let mut out = Vec::new();
        self.chat_block -= delta;
        self.attack_repeat -= delta;
        self.gather_repeat -= delta;

        if self.just_pressed(&input, Key::ENTER) && self.chat_block <= 0.0 {
            out.extend(self.release());
            out.push(Command::FocusChat);
            return out;
        }

        let k = |key: Key| input.is_physical_key_pressed(key) as i32;
        let dx = (k(Key::D) | k(Key::RIGHT)) - (k(Key::A) | k(Key::LEFT));
        let dy = (k(Key::S) | k(Key::DOWN)) - (k(Key::W) | k(Key::UP));
        if (dx, dy) != self.last_dir {
            self.last_dir = (dx, dy);
            out.push(Command::Move(dx, dy));
        }
        if input.is_physical_key_pressed(Key::SPACE) && self.attack_repeat <= 0.0 {
            self.attack_repeat = 0.25;
            out.push(Command::Attack);
        }
        if input.is_physical_key_pressed(Key::E) && self.gather_repeat <= 0.0 {
            self.gather_repeat = 0.25;
            out.push(Command::Gather);
        }
        if self.just_pressed(&input, Key::Q) {
            out.push(Command::Potion);
        }
        if self.just_pressed(&input, Key::C) {
            out.push(Command::ToggleCrafting);
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
