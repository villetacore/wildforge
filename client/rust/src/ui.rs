//! Builds the login screen and the in-game HUD out of plain Godot controls.
//! Button/list signals push `UiEvent`s into a shared queue that the game
//! node drains every frame.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use godot::classes::control::{FocusMode, MouseFilter};
use godot::classes::{
    Button, CanvasLayer, ColorRect, Control, ItemList, Label, LineEdit, PanelContainer, ProgressBar, RichTextLabel,
    StyleBoxFlat, TextureRect, VBoxContainer,
};
use godot::prelude::*;

pub enum UiEvent {
    Login,
    Register,
    Chat(String),
    UseItem(i64),
    DropItem(i64),
    Unequip(&'static str),
}

pub type UiQueue = Rc<RefCell<VecDeque<UiEvent>>>;

pub const SCREEN_W: f32 = 1280.0;
pub const SCREEN_H: f32 = 720.0;

fn panel(pos: Vector2, size: Vector2, alpha: f32) -> Gd<PanelContainer> {
    let mut p = PanelContainer::new_alloc();
    let mut style = StyleBoxFlat::new_gd();
    style.set_bg_color(Color::from_rgba(0.08, 0.09, 0.12, alpha));
    style.set_corner_radius_all(6);
    style.set_content_margin_all(10.0);
    p.add_theme_stylebox_override("panel", &style);
    p.set_position(pos);
    p.set_size(size);
    p
}

fn label(text: &str, size: i32) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
    l.add_theme_font_size_override("font_size", size);
    l
}

fn line_edit(placeholder: &str, text: &str) -> Gd<LineEdit> {
    let mut e = LineEdit::new_alloc();
    e.set_placeholder(placeholder);
    e.set_text(text);
    e.set_custom_minimum_size(Vector2::new(0.0, 34.0));
    e
}

fn button(text: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(text);
    b.set_focus_mode(FocusMode::NONE); // so Space never "clicks" a button
    b.set_custom_minimum_size(Vector2::new(0.0, 34.0));
    b
}

fn bar(color: Color) -> Gd<ProgressBar> {
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

// ------------------------------------------------------------------ login

pub struct LoginUi {
    pub layer: Gd<CanvasLayer>,
    pub server: Gd<LineEdit>,
    pub username: Gd<LineEdit>,
    pub password: Gd<LineEdit>,
    pub status: Gd<Label>,
}

pub fn build_login(q: &UiQueue) -> LoginUi {
    let mut layer = CanvasLayer::new_alloc();
    let mut bg = ColorRect::new_alloc();
    bg.set_color(Color::from_rgb(0.07, 0.1, 0.09));
    bg.set_size(Vector2::new(SCREEN_W, SCREEN_H));
    layer.add_child(&bg);

    let mut p = panel(Vector2::new(SCREEN_W / 2.0 - 200.0, 150.0), Vector2::new(400.0, 0.0), 0.95);
    let mut v = VBoxContainer::new_alloc();
    v.add_theme_constant_override("separation", 8);

    let mut title = label("WILDFORGE", 40);
    title.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
    v.add_child(&title);
    let mut sub = label("top-down online adventure", 14);
    sub.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
    sub.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
    v.add_child(&sub);

    v.add_child(&label("Server", 13));
    let server = line_edit("ws://host:7777/ws", "ws://127.0.0.1:7777/ws");
    v.add_child(&server);
    v.add_child(&label("Username", 13));
    let username = line_edit("3-16 letters, digits or _", "");
    v.add_child(&username);
    v.add_child(&label("Password", 13));
    let mut password = line_edit("at least 4 characters", "");
    password.set_secret(true);
    v.add_child(&password);

    let login = button("Log in");
    let register = button("Create account");
    v.add_child(&login);
    v.add_child(&register);

    let mut status = label("", 14);
    status.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD);
    status.set_custom_minimum_size(Vector2::new(0.0, 40.0));
    v.add_child(&status);

    p.add_child(&v);
    layer.add_child(&p);

    let qc = q.clone();
    login.signals().pressed().connect(move || qc.borrow_mut().push_back(UiEvent::Login));
    let qc = q.clone();
    register.signals().pressed().connect(move || qc.borrow_mut().push_back(UiEvent::Register));
    let qc = q.clone();
    password.signals().text_submitted().connect(move |_t: GString| qc.borrow_mut().push_back(UiEvent::Login));

    LoginUi { layer, server, username, password, status }
}

// -------------------------------------------------------------------- hud

pub struct Hud {
    pub layer: Gd<CanvasLayer>,
    pub name: Gd<Label>,
    pub hp_label: Gd<Label>,
    pub hp: Gd<ProgressBar>,
    pub xp_label: Gd<Label>,
    pub xp: Gd<ProgressBar>,
    pub stats: Gd<Label>,
    pub equip: Gd<Label>,
    pub minimap: Gd<TextureRect>,
    pub marker: Gd<ColorRect>,
    pub inventory: Gd<ItemList>,
    pub chat_log: Gd<RichTextLabel>,
    pub chat_input: Gd<LineEdit>,
}

pub fn build_hud(q: &UiQueue) -> Hud {
    let mut layer = CanvasLayer::new_alloc();

    // Character panel, top left.
    let mut p = panel(Vector2::new(10.0, 10.0), Vector2::new(300.0, 0.0), 0.75);
    let mut v = VBoxContainer::new_alloc();
    v.add_theme_constant_override("separation", 3);
    let name = label("", 18);
    let hp_label = label("", 13);
    let hp = bar(Color::from_rgb(0.85, 0.2, 0.2));
    let xp_label = label("", 13);
    let xp = bar(Color::from_rgb(0.3, 0.6, 0.95));
    let stats = label("", 13);
    let equip = label("", 13);
    for c in [name.clone().upcast::<Control>(), hp_label.clone().upcast(), hp.clone().upcast(), xp_label.clone().upcast(), xp.clone().upcast(), stats.clone().upcast(), equip.clone().upcast()] {
        v.add_child(&c);
    }
    p.add_child(&v);
    layer.add_child(&p);

    // Minimap, top right.
    let mut mm_panel = panel(Vector2::new(SCREEN_W - 222.0, 10.0), Vector2::new(212.0, 212.0), 0.75);
    mm_panel.add_theme_stylebox_override("panel", &{
        let mut s = StyleBoxFlat::new_gd();
        s.set_bg_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        s.set_content_margin_all(6.0);
        s
    });
    let mut minimap = TextureRect::new_alloc();
    minimap.set_expand_mode(godot::classes::texture_rect::ExpandMode::IGNORE_SIZE);
    minimap.set_custom_minimum_size(Vector2::new(200.0, 200.0));
    minimap.set_texture_filter(godot::classes::canvas_item::TextureFilter::NEAREST);
    let mut marker = ColorRect::new_alloc();
    marker.set_color(Color::from_rgb(1.0, 0.9, 0.2));
    marker.set_size(Vector2::new(5.0, 5.0));
    minimap.add_child(&marker);
    mm_panel.add_child(&minimap);
    layer.add_child(&mm_panel);

    // Inventory, right.
    let mut ip = panel(Vector2::new(SCREEN_W - 222.0, 232.0), Vector2::new(212.0, 0.0), 0.75);
    let mut iv = VBoxContainer::new_alloc();
    iv.add_child(&label("Inventory", 16));
    let mut hint = label("double-click: use/equip\nright-click: drop", 11);
    hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
    iv.add_child(&hint);
    let mut inventory = ItemList::new_alloc();
    inventory.set_custom_minimum_size(Vector2::new(0.0, 280.0));
    inventory.set_focus_mode(FocusMode::NONE);
    inventory.set_allow_rmb_select(true);
    iv.add_child(&inventory);
    let unequip_w = button("Unequip weapon");
    let unequip_a = button("Unequip armor");
    iv.add_child(&unequip_w);
    iv.add_child(&unequip_a);
    ip.add_child(&iv);
    layer.add_child(&ip);

    // Chat, bottom left.
    let mut cp = panel(Vector2::new(10.0, SCREEN_H - 250.0), Vector2::new(440.0, 240.0), 0.6);
    let mut cv = VBoxContainer::new_alloc();
    let mut chat_log = RichTextLabel::new_alloc();
    chat_log.set_use_bbcode(true);
    chat_log.set_scroll_follow(true);
    chat_log.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
    chat_log.add_theme_font_size_override("normal_font_size", 13);
    chat_log.set_focus_mode(FocusMode::NONE);
    cv.add_child(&chat_log);
    let chat_input = line_edit("Press Enter to chat  (/who, /stats)", "");
    cv.add_child(&chat_input);
    cp.add_child(&cv);
    layer.add_child(&cp);

    // Controls help, bottom right.
    let mut help = label("WASD move · Space attack · Q potion · 1-9 use item · Enter chat", 12);
    help.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7));
    help.set_position(Vector2::new(SCREEN_W - 470.0, SCREEN_H - 26.0));
    help.set_mouse_filter(MouseFilter::IGNORE);
    layer.add_child(&help);

    let qc = q.clone();
    inventory.signals().item_activated().connect(move |i: i64| qc.borrow_mut().push_back(UiEvent::UseItem(i)));
    let qc = q.clone();
    inventory.signals().item_clicked().connect(move |i: i64, _pos: Vector2, button: i64| {
        if button == 2 {
            qc.borrow_mut().push_back(UiEvent::DropItem(i));
        }
    });
    let qc = q.clone();
    unequip_w.signals().pressed().connect(move || qc.borrow_mut().push_back(UiEvent::Unequip("weapon")));
    let qc = q.clone();
    unequip_a.signals().pressed().connect(move || qc.borrow_mut().push_back(UiEvent::Unequip("armor")));
    let qc = q.clone();
    chat_input.signals().text_submitted().connect(move |t: GString| qc.borrow_mut().push_back(UiEvent::Chat(t.to_string())));

    Hud { layer, name, hp_label, hp, xp_label, xp, stats, equip, minimap, marker, inventory, chat_log, chat_input }
}
