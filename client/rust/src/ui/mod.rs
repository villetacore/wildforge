//! Screen-space UI built from plain Godot controls. Widget signals push
//! `UiEvent`s into a shared queue that the app drains every frame, so UI code
//! never calls back into game logic directly.

pub mod crafting;
pub mod hud;
pub mod login;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use godot::classes::control::FocusMode;
use godot::classes::{Button, Label, LineEdit, PanelContainer, ProgressBar, StyleBoxFlat};
use godot::prelude::*;

pub enum UiEvent {
    Login,
    Register,
    Chat(String),
    UseItem(i64),
    DropItem(i64),
    Unequip(&'static str),
    Craft(String),
    CraftSelected,
    ToggleCrafting,
}

pub type UiQueue = Rc<RefCell<VecDeque<UiEvent>>>;

pub fn push(q: &UiQueue, ev: UiEvent) {
    q.borrow_mut().push_back(ev);
}

pub const SCREEN_W: f32 = 1280.0;
pub const SCREEN_H: f32 = 720.0;

pub fn panel(pos: Vector2, size: Vector2, alpha: f32) -> Gd<PanelContainer> {
    let mut p = PanelContainer::new_alloc();
    let mut style = StyleBoxFlat::new_gd();
    style.set_bg_color(Color::from_rgba(0.08, 0.065, 0.055, alpha));
    style.set_corner_radius_all(4);
    style.set_content_margin_all(10.0);
    style.set_border_width_all(1);
    style.set_border_color(Color::from_rgba(0.73, 0.54, 0.29, 0.85));
    p.add_theme_stylebox_override("panel", &style);
    p.set_position(pos);
    p.set_size(size);
    p
}

pub fn label(text: &str, size: i32) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
    l.add_theme_font_size_override("font_size", size);
    if size >= 16 {
        l.add_theme_color_override("font_color", Color::from_rgb(0.99, 0.84, 0.52));
    }
    l
}

pub fn line_edit(placeholder: &str, text: &str) -> Gd<LineEdit> {
    let mut e = LineEdit::new_alloc();
    e.set_placeholder(placeholder);
    e.set_text(text);
    e.set_custom_minimum_size(Vector2::new(0.0, 34.0));
    e
}

pub fn button(text: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(text);
    b.set_focus_mode(FocusMode::NONE); // so Space never "clicks" a button
    b.set_custom_minimum_size(Vector2::new(0.0, 32.0));
    b
}

pub fn bar(color: Color) -> Gd<ProgressBar> {
    let mut b = ProgressBar::new_alloc();
    b.set_show_percentage(false);
    b.set_custom_minimum_size(Vector2::new(0.0, 14.0));
    let mut fill = StyleBoxFlat::new_gd();
    fill.set_bg_color(color);
    fill.set_corner_radius_all(3);
    let mut bg = StyleBoxFlat::new_gd();
    bg.set_bg_color(Color::from_rgba(0.0, 0.0, 0.0, 0.5));
    bg.set_corner_radius_all(3);
    b.add_theme_stylebox_override("fill", &fill);
    b.add_theme_stylebox_override("background", &bg);
    b
}

/// Escapes text for a BBCode RichTextLabel.
pub fn escape(s: &str) -> String {
    s.replace('[', "[lb]")
}
