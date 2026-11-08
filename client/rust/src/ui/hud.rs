//! In-game HUD: character panel, minimap, inventory and chat.

use godot::classes::control::{FocusMode, MouseFilter, SizeFlags};
use godot::classes::texture_rect::ExpandMode;
use godot::classes::{
    CanvasLayer, ColorRect, Control, ImageTexture, ItemList, Label, LineEdit, ProgressBar,
    RichTextLabel, StyleBoxFlat, TextureRect, VBoxContainer,
};
use godot::prelude::*;

use super::{bar, button, label, line_edit, panel, push, UiEvent, UiQueue, SCREEN_H, SCREEN_W};
use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::Stack;
use crate::session::Session;

const MINIMAP: f32 = 200.0;

pub struct Hud {
    pub layer: Gd<CanvasLayer>,
    name: Gd<Label>,
    hp_label: Gd<Label>,
    hp: Gd<ProgressBar>,
    xp_label: Gd<Label>,
    xp: Gd<ProgressBar>,
    stats: Gd<Label>,
    equip: Gd<Label>,
    marker: Gd<ColorRect>,
    inventory: Gd<ItemList>,
    shown_inventory: Vec<Stack>,
    chat_log: Gd<RichTextLabel>,
    pub chat_input: Gd<LineEdit>,
}

impl Hud {
    pub fn build(q: &UiQueue, minimap_tex: Option<Gd<ImageTexture>>) -> Hud {
        let mut layer = CanvasLayer::new_alloc();

        // Character panel, top left.
        let mut p = panel(Vector2::new(10.0, 10.0), Vector2::new(310.0, 0.0), 0.75);
        let mut v = VBoxContainer::new_alloc();
        v.add_theme_constant_override("separation", 3);
        let name = label("", 18);
        let hp_label = label("", 13);
        let hp = bar(Color::from_rgb(0.85, 0.2, 0.2));
        let xp_label = label("", 13);
        let xp = bar(Color::from_rgb(0.3, 0.6, 0.95));
        let stats = label("", 13);
        let equip = label("", 13);
        let parts: [Gd<Control>; 7] = [
            name.clone().upcast(),
            hp_label.clone().upcast(),
            hp.clone().upcast(),
            xp_label.clone().upcast(),
            xp.clone().upcast(),
            stats.clone().upcast(),
            equip.clone().upcast(),
        ];
        for c in parts {
            v.add_child(&c);
        }
        p.add_child(&v);
        layer.add_child(&p);

        // Minimap, top right.
        let mut mm_panel = panel(
            Vector2::new(SCREEN_W - MINIMAP - 22.0, 10.0),
            Vector2::new(MINIMAP + 12.0, MINIMAP + 12.0),
            0.75,
        );
        let mut mm_style = StyleBoxFlat::new_gd();
        mm_style.set_bg_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        mm_style.set_content_margin_all(6.0);
        mm_style.set_corner_radius_all(8);
        mm_panel.add_theme_stylebox_override("panel", &mm_style);
        let mut minimap = TextureRect::new_alloc();
        minimap.set_expand_mode(ExpandMode::IGNORE_SIZE);
        minimap.set_custom_minimum_size(Vector2::new(MINIMAP, MINIMAP));
        minimap.set_texture_filter(godot::classes::canvas_item::TextureFilter::NEAREST);
        if let Some(t) = minimap_tex {
            minimap.set_texture(&t);
        }
        let mut marker = ColorRect::new_alloc();
        marker.set_color(Color::from_rgb(1.0, 0.9, 0.2));
        marker.set_size(Vector2::new(5.0, 5.0));
        minimap.add_child(&marker);
        mm_panel.add_child(&minimap);
        layer.add_child(&mm_panel);

        // Inventory, right.
        let mut ip = panel(
            Vector2::new(SCREEN_W - MINIMAP - 22.0, MINIMAP + 32.0),
            Vector2::new(MINIMAP + 12.0, 0.0),
            0.75,
        );
        let mut iv = VBoxContainer::new_alloc();
        iv.add_child(&label("Inventory", 16));
        let mut hint = label("double-click: use/equip · right-click: drop", 10);
        hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
        iv.add_child(&hint);
        let mut inventory = ItemList::new_alloc();
        inventory.set_custom_minimum_size(Vector2::new(0.0, 270.0));
        inventory.set_focus_mode(FocusMode::NONE);
        inventory.set_allow_rmb_select(true);
        inventory.add_theme_font_size_override("font_size", 13);
        iv.add_child(&inventory);
        let unequip_w = button("Unequip weapon");
        let unequip_a = button("Unequip armor");
        let craft = button("Crafting (C)");
        iv.add_child(&craft);
        iv.add_child(&unequip_w);
        iv.add_child(&unequip_a);
        ip.add_child(&iv);
        layer.add_child(&ip);

        // Chat, bottom left.
        let mut cp = panel(
            Vector2::new(10.0, SCREEN_H - 250.0),
            Vector2::new(440.0, 240.0),
            0.6,
        );
        let mut cv = VBoxContainer::new_alloc();
        let mut chat_log = RichTextLabel::new_alloc();
        chat_log.set_use_bbcode(true);
        chat_log.set_scroll_follow(true);
        chat_log.set_v_size_flags(SizeFlags::EXPAND_FILL);
        chat_log.add_theme_font_size_override("normal_font_size", 13);
        chat_log.set_focus_mode(FocusMode::NONE);
        cv.add_child(&chat_log);
        let chat_input = line_edit("Press Enter to chat  (/who, /stats, /help)", "");
        cv.add_child(&chat_input);
        cp.add_child(&cv);
        layer.add_child(&cp);

        let mut help = label("WASD move · Space attack · E gather · C craft · Q potion · 1-9 use · wheel zoom · Enter chat", 12);
        help.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.75));
        help.set_position(Vector2::new(SCREEN_W - 640.0, SCREEN_H - 24.0));
        help.set_mouse_filter(MouseFilter::IGNORE);
        layer.add_child(&help);

        let qc = q.clone();
        inventory
            .signals()
            .item_activated()
            .connect(move |i: i64| push(&qc, UiEvent::UseItem(i)));
        let qc = q.clone();
        inventory
            .signals()
            .item_clicked()
            .connect(move |i: i64, _pos: Vector2, button: i64| {
                if button == 2 {
                    push(&qc, UiEvent::DropItem(i));
                }
            });
        let qc = q.clone();
        unequip_w
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::Unequip("weapon")));
        let qc = q.clone();
        unequip_a
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::Unequip("armor")));
        let qc = q.clone();
        craft
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::ToggleCrafting));
        let qc = q.clone();
        chat_input
            .signals()
            .text_submitted()
            .connect(move |t: GString| push(&qc, UiEvent::Chat(t.to_string())));

        Hud {
            layer,
            name,
            hp_label,
            hp,
            xp_label,
            xp,
            stats,
            equip,
            marker,
            inventory,
            shown_inventory: Vec::new(),
            chat_log,
            chat_input,
        }
    }

    pub fn chat_line(&mut self, bbcode: &str) {
        self.chat_log.append_text(&format!("{bbcode}\n"));
    }

    pub fn chat_focused(&self) -> bool {
        self.chat_input.has_focus()
    }

    /// Refreshes all widgets from the latest server state.
    pub fn update(&mut self, s: &Session, lib: &ContentLibrary) {
        let you = &s.you;
        let cat = &s.catalog;
        self.name
            .set_text(&format!("{}  ·  Level {}", s.my_name, you.level));
        self.hp_label
            .set_text(&format!("HP {} / {}", you.hp, you.max_hp));
        self.hp.set_max(you.max_hp.max(1.0) as f64);
        self.hp.set_value(you.hp as f64);
        self.xp_label
            .set_text(&format!("XP {} / {}", you.xp, you.xp_next));
        self.xp.set_max(you.xp_next.max(1) as f64);
        self.xp.set_value(you.xp as f64);
        self.stats.set_text(&format!(
            "ATK {}   DEF {}   Gold {}   Kills {}   Online {}",
            you.attack, you.defense, you.gold, you.kills, you.online
        ));
        let or_dash = |id: &str| {
            if id.is_empty() {
                "-".to_string()
            } else {
                cat.item_name(id)
            }
        };
        self.equip.set_text(&format!(
            "Weapon: {}\nArmor: {}",
            or_dash(&you.weapon),
            or_dash(&you.armor)
        ));
        let n = s.size.max(1) as f32;
        self.marker.set_position(Vector2::new(
            you.x / n * MINIMAP - 2.5,
            you.y / n * MINIMAP - 2.5,
        ));

        if you.inventory != self.shown_inventory {
            self.inventory.clear();
            for (i, st) in you.inventory.iter().enumerate() {
                let def = cat.item(&st.item);
                let name = cat.item_name(&st.item);
                let key = if i < 9 {
                    format!("{}  ", i + 1)
                } else {
                    "    ".into()
                };
                let text = if st.qty > 1 {
                    format!("{key}{name} ×{}", st.qty)
                } else {
                    format!("{key}{name}")
                };
                let (kind, color) = def
                    .map(|d| (d.kind.as_str(), parse_color(&d.color)))
                    .unwrap_or(("material", Color::WHITE));
                let icon = lib.icon(&st.item, kind, color);
                self.inventory.add_item_ex(&text).icon(&icon).done();
                if let Some(d) = def {
                    self.inventory.set_item_tooltip(i as i32, &d.desc);
                }
            }
            self.shown_inventory = you.inventory.clone();
        }
    }
}
