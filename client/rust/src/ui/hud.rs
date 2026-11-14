//! In-game HUD: character frame, target frame, minimap and world map, spell
//! bar, item bar, inventory with equipment, and chat.

use godot::classes::control::{FocusMode, MouseFilter, SizeFlags};
use godot::classes::text_server::AutowrapMode;
use godot::classes::texture_rect::ExpandMode;
use godot::classes::{
    Button, CanvasLayer, ColorRect, Control, HBoxContainer, ImageTexture, ItemList, Label,
    LineEdit, PanelContainer, ProgressBar, RichTextLabel, StyleBoxFlat, TextureRect, VBoxContainer,
};
use godot::prelude::*;

use super::{bar, button, label, line_edit, panel, push, UiEvent, UiQueue, SCREEN_H, SCREEN_W};
use crate::content::{parse_color, ContentLibrary};
use crate::net::protocol::Mark;
use crate::session::Session;

const MINIMAP: f32 = 190.0;
const MINIMAP_SPAN: f32 = 140.0; // tiles shown across the minimap
const BIGMAP: f32 = 600.0;
const SLOTS: [&str; 9] = [
    "weapon", "offhand", "head", "chest", "legs", "hands", "feet", "ring", "amulet",
];
const SPELL_KEYS: [&str; 6] = ["R", "F", "G", "T", "Z", "X"];

pub struct Hud {
    pub layer: Gd<CanvasLayer>,
    name: Gd<Label>,
    hp_label: Gd<Label>,
    hp: Gd<ProgressBar>,
    mana_label: Gd<Label>,
    mana: Gd<ProgressBar>,
    xp: Gd<ProgressBar>,
    mood: Gd<ProgressBar>,
    stats: Gd<Label>,
    statuses: Gd<RichTextLabel>,
    zone: Gd<Label>,
    clock: Gd<Label>,
    target_panel: Gd<PanelContainer>,
    target_name: Gd<Label>,
    target_hp: Gd<ProgressBar>,
    minimap: Gd<TextureRect>,
    minimap_tex: Option<Gd<ImageTexture>>,
    marker: Gd<ColorRect>,
    mark_dots: Vec<Gd<ColorRect>>,
    shown_marks: Vec<Mark>,
    bigmap: Gd<PanelContainer>,
    bigmap_rect: Gd<TextureRect>,
    bigmap_marker: Gd<ColorRect>,
    bigmap_dots: Vec<Gd<Control>>,
    inv_panel: Gd<PanelContainer>,
    inventory: Gd<ItemList>,
    equipment: Gd<ItemList>,
    slots: Vec<Gd<Button>>,
    spell_buttons: Vec<Gd<Button>>,
    shown_version: u64,
    chat_log: Gd<RichTextLabel>,
    pub chat_input: Gd<LineEdit>,
}

fn small_bar(color: Color, h: f32) -> Gd<ProgressBar> {
    let mut b = bar(color);
    b.set_custom_minimum_size(Vector2::new(0.0, h));
    b
}

impl Hud {
    pub fn build(q: &UiQueue, minimap_tex: Option<Gd<ImageTexture>>) -> Hud {
        let mut layer = CanvasLayer::new_alloc();

        // Character frame, top left.
        let mut p = panel(Vector2::new(10.0, 10.0), Vector2::new(320.0, 0.0), 0.8);
        let mut v = VBoxContainer::new_alloc();
        v.add_theme_constant_override("separation", 2);
        let name = label("", 18);
        let hp_label = label("", 12);
        let hp = small_bar(Color::from_rgb(0.82, 0.18, 0.18), 16.0);
        let mana_label = label("", 12);
        let mana = small_bar(Color::from_rgb(0.22, 0.42, 0.9), 12.0);
        let xp = small_bar(Color::from_rgb(0.6, 0.38, 0.9), 6.0);
        let mood = small_bar(Color::from_rgb(1.0, 0.55, 0.75), 5.0);
        let mut stats = label("", 12);
        stats.set_autowrap_mode(AutowrapMode::WORD);
        let mut statuses = RichTextLabel::new_alloc();
        statuses.set_use_bbcode(true);
        statuses.set_fit_content(true);
        statuses.set_scroll_active(false);
        statuses.add_theme_font_size_override("normal_font_size", 11);
        statuses.set_mouse_filter(MouseFilter::IGNORE);
        for c in [
            name.clone().upcast::<Control>(),
            hp_label.clone().upcast(),
            hp.clone().upcast(),
            mana_label.clone().upcast(),
            mana.clone().upcast(),
            xp.clone().upcast(),
            mood.clone().upcast(),
            stats.clone().upcast(),
            statuses.clone().upcast(),
        ] {
            v.add_child(&c);
        }
        p.add_child(&v);
        layer.add_child(&p);

        // Zone name and target frame, top centre.
        let mut zone = label("", 20);
        zone.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        zone.set_position(Vector2::new(SCREEN_W / 2.0 - 250.0, 8.0));
        zone.set_size(Vector2::new(500.0, 28.0));
        zone.add_theme_constant_override("outline_size", 6);
        zone.add_theme_color_override("font_outline_color", Color::from_rgb(0.0, 0.0, 0.0));
        zone.set_mouse_filter(MouseFilter::IGNORE);
        layer.add_child(&zone);
        let mut target_panel = panel(
            Vector2::new(SCREEN_W / 2.0 - 160.0, 40.0),
            Vector2::new(320.0, 0.0),
            0.8,
        );
        let mut tv = VBoxContainer::new_alloc();
        let mut target_name = label("", 14);
        target_name.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        let target_hp = small_bar(Color::from_rgb(0.85, 0.22, 0.18), 14.0);
        tv.add_child(&target_name);
        tv.add_child(&target_hp);
        target_panel.add_child(&tv);
        target_panel.set_visible(false);
        layer.add_child(&target_panel);

        // Minimap, top right.
        let mut mm_panel = panel(
            Vector2::new(SCREEN_W - MINIMAP - 22.0, 10.0),
            Vector2::new(MINIMAP + 12.0, MINIMAP + 12.0),
            0.75,
        );
        let mut mm_style = StyleBoxFlat::new_gd();
        mm_style.set_bg_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        mm_style.set_content_margin_all(6.0);
        mm_style.set_corner_radius_all(MINIMAP as i32 / 2 + 6);
        mm_style.set_border_width_all(3);
        mm_style.set_border_color(Color::from_rgb(0.67, 0.49, 0.26));
        mm_panel.add_theme_stylebox_override("panel", &mm_style);
        let mut clip = Control::new_alloc();
        clip.set_custom_minimum_size(Vector2::new(MINIMAP, MINIMAP));
        clip.set_clip_contents(true);
        let mut minimap = TextureRect::new_alloc();
        minimap.set_expand_mode(ExpandMode::IGNORE_SIZE);
        minimap.set_texture_filter(godot::classes::canvas_item::TextureFilter::NEAREST);
        if let Some(t) = &minimap_tex {
            minimap.set_texture(t);
        }
        clip.add_child(&minimap);
        let mut marker = ColorRect::new_alloc();
        marker.set_color(Color::from_rgb(1.0, 0.95, 0.3));
        marker.set_size(Vector2::new(7.0, 7.0));
        marker.set_position(Vector2::new(MINIMAP / 2.0 - 3.5, MINIMAP / 2.0 - 3.5));
        clip.add_child(&marker);
        mm_panel.add_child(&clip);
        layer.add_child(&mm_panel);
        let mut clock = label("", 12);
        clock.set_position(Vector2::new(SCREEN_W - MINIMAP - 18.0, MINIMAP + 26.0));
        clock.set_size(Vector2::new(MINIMAP, 18.0));
        clock.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        layer.add_child(&clock);

        // World map (M).
        let mut bigmap = panel(
            Vector2::new(
                SCREEN_W / 2.0 - BIGMAP / 2.0 - 8.0,
                SCREEN_H / 2.0 - BIGMAP / 2.0 - 8.0,
            ),
            Vector2::new(BIGMAP + 16.0, BIGMAP + 16.0),
            0.95,
        );
        let mut big_inner = Control::new_alloc();
        big_inner.set_custom_minimum_size(Vector2::new(BIGMAP, BIGMAP));
        let mut bigmap_rect = TextureRect::new_alloc();
        bigmap_rect.set_expand_mode(ExpandMode::IGNORE_SIZE);
        bigmap_rect.set_size(Vector2::new(BIGMAP, BIGMAP));
        if let Some(t) = &minimap_tex {
            bigmap_rect.set_texture(t);
        }
        big_inner.add_child(&bigmap_rect);
        let mut bigmap_marker = ColorRect::new_alloc();
        bigmap_marker.set_color(Color::from_rgb(1.0, 0.95, 0.3));
        bigmap_marker.set_size(Vector2::new(8.0, 8.0));
        big_inner.add_child(&bigmap_marker);
        bigmap.add_child(&big_inner);
        bigmap.set_visible(false);
        layer.add_child(&bigmap);

        // Inventory and equipment, right (I).
        let mut inv_panel = panel(
            Vector2::new(SCREEN_W - 262.0, MINIMAP + 48.0),
            Vector2::new(252.0, 0.0),
            0.8,
        );
        let mut iv = VBoxContainer::new_alloc();
        iv.add_child(&label("Equipment", 15));
        let mut equipment = ItemList::new_alloc();
        equipment.set_custom_minimum_size(Vector2::new(0.0, 120.0));
        equipment.set_focus_mode(FocusMode::NONE);
        equipment.add_theme_font_size_override("font_size", 11);
        iv.add_child(&equipment);
        iv.add_child(&label("Inventory", 15));
        let mut hint = label("double-click: use/equip · right-click: drop", 10);
        hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
        iv.add_child(&hint);
        let mut inventory = ItemList::new_alloc();
        inventory.set_custom_minimum_size(Vector2::new(0.0, 175.0));
        inventory.set_focus_mode(FocusMode::NONE);
        inventory.set_allow_rmb_select(true);
        inventory.add_theme_font_size_override("font_size", 12);
        iv.add_child(&inventory);
        let mut row1 = HBoxContainer::new_alloc();
        let mut row2 = HBoxContainer::new_alloc();
        for (i, (text, ev)) in [
            ("Craft C", 0usize),
            ("Hero K", 1),
            ("Spells B", 2),
            ("Pets H", 3),
            ("Rep J", 4),
            ("Jobs L", 5),
        ]
        .into_iter()
        .enumerate()
        {
            let mut b = button(text);
            b.set_h_size_flags(SizeFlags::EXPAND_FILL);
            b.add_theme_font_size_override("font_size", 11);
            let qc = q.clone();
            b.signals().pressed().connect(move || {
                push(
                    &qc,
                    match ev {
                        0 => UiEvent::ToggleCrafting,
                        1 => UiEvent::Menu("char"),
                        2 => UiEvent::Menu("spells"),
                        3 => UiEvent::Menu("pets"),
                        4 => UiEvent::Menu("rep"),
                        _ => UiEvent::Menu("journal"),
                    },
                )
            });
            if i < 3 {
                row1.add_child(&b)
            } else {
                row2.add_child(&b)
            }
        }
        iv.add_child(&row1);
        iv.add_child(&row2);
        inv_panel.add_child(&iv);
        layer.add_child(&inv_panel);

        // Chat, bottom left.
        let mut cp = panel(
            Vector2::new(10.0, SCREEN_H - 230.0),
            Vector2::new(420.0, 220.0),
            0.6,
        );
        let mut cv = VBoxContainer::new_alloc();
        let mut chat_log = RichTextLabel::new_alloc();
        chat_log.set_use_bbcode(true);
        chat_log.set_scroll_follow(true);
        chat_log.set_v_size_flags(SizeFlags::EXPAND_FILL);
        chat_log.add_theme_font_size_override("normal_font_size", 12);
        chat_log.set_focus_mode(FocusMode::NONE);
        cv.add_child(&chat_log);
        let chat_input = line_edit("Enter to chat  (/who /stats /help /stuck)", "");
        cv.add_child(&chat_input);
        cp.add_child(&cv);
        layer.add_child(&cp);

        // Action bars, bottom centre: spells above items.
        let mut bars = panel(Vector2::new(442.0, SCREEN_H - 118.0), Vector2::ZERO, 0.88);
        let mut bv = VBoxContainer::new_alloc();
        let mut spell_row = HBoxContainer::new_alloc();
        let mut spell_buttons = Vec::new();
        for i in 0..6 {
            let mut b = button(SPELL_KEYS[i]);
            b.set_custom_minimum_size(Vector2::new(88.0, 40.0));
            b.add_theme_font_size_override("font_size", 11);
            b.set_clip_text(true);
            let qc = q.clone();
            b.signals()
                .pressed()
                .connect(move || push(&qc, UiEvent::Cast(i)));
            spell_row.add_child(&b);
            spell_buttons.push(b);
        }
        bv.add_child(&spell_row);
        let mut row = HBoxContainer::new_alloc();
        let mut slots = Vec::new();
        for i in 0..9 {
            let mut slot = button(&(i + 1).to_string());
            slot.set_custom_minimum_size(Vector2::new(56.0, 36.0));
            slot.add_theme_constant_override("icon_max_width", 24);
            slot.set_disabled(true);
            let qc = q.clone();
            slot.signals()
                .pressed()
                .connect(move || push(&qc, UiEvent::UseItem(i)));
            row.add_child(&slot);
            slots.push(slot);
        }
        bv.add_child(&row);
        bars.add_child(&bv);
        layer.add_child(&bars);

        let mut help = label("WASD move · RMB drag camera · wheel zoom · click target · Space attack · R F G T Z X spells · E interact · Y tame · Q potion · Tab target · I bag · M map · Esc close", 11);
        help.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7));
        help.set_position(Vector2::new(442.0, SCREEN_H - 20.0));
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
        equipment.signals().item_activated().connect(move |i: i64| {
            if let Some(slot) = SLOTS.get(i as usize) {
                push(&qc, UiEvent::Unequip(slot.to_string()));
            }
        });
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
            mana_label,
            mana,
            xp,
            mood,
            stats,
            statuses,
            zone,
            clock,
            target_panel,
            target_name,
            target_hp,
            minimap,
            minimap_tex,
            marker,
            mark_dots: Vec::new(),
            shown_marks: Vec::new(),
            bigmap,
            bigmap_rect,
            bigmap_marker,
            bigmap_dots: Vec::new(),
            inv_panel,
            inventory,
            equipment,
            slots,
            spell_buttons,
            shown_version: u64::MAX,
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

    pub fn toggle_inventory(&mut self) {
        let v = !self.inv_panel.is_visible();
        self.inv_panel.set_visible(v);
    }

    pub fn toggle_map(&mut self) {
        let v = !self.bigmap.is_visible();
        self.bigmap.set_visible(v);
    }

    pub fn close_map(&mut self) -> bool {
        let was = self.bigmap.is_visible();
        self.bigmap.set_visible(false);
        was
    }

    /// Shows the target frame for an enemy (name, hp).
    pub fn set_target(&mut self, target: Option<(String, f32, f32, Color)>) {
        match target {
            Some((name, hp, max, color)) => {
                self.target_panel.set_visible(true);
                self.target_name.set_text(&name);
                self.target_name
                    .add_theme_color_override("font_color", color);
                self.target_hp.set_max(max.max(1.0) as f64);
                self.target_hp.set_value(hp as f64);
            }
            None => self.target_panel.set_visible(false),
        }
    }

    fn update_marks(&mut self, s: &Session) {
        if s.you.marks == self.shown_marks {
            return;
        }
        for mut d in self.mark_dots.drain(..) {
            d.queue_free();
        }
        for mut d in self.bigmap_dots.drain(..) {
            d.queue_free();
        }
        let n = s.size.max(1) as f32;
        for m in &s.you.marks {
            let mut dot = ColorRect::new_alloc();
            dot.set_color(parse_color(&m.color));
            dot.set_size(Vector2::new(6.0, 6.0));
            dot.set_mouse_filter(MouseFilter::IGNORE);
            dot.set_tooltip_text(&m.label);
            self.minimap.add_child(&dot);
            self.mark_dots.push(dot);
            let mut big = ColorRect::new_alloc();
            big.set_color(parse_color(&m.color));
            big.set_size(Vector2::new(9.0, 9.0));
            big.set_position(Vector2::new(m.x / n * BIGMAP - 4.5, m.y / n * BIGMAP - 4.5));
            big.set_tooltip_text(&m.label);
            self.bigmap_rect.add_child(&big);
            let mut text = label(&m.label, 10);
            text.set_position(Vector2::new(m.x / n * BIGMAP + 6.0, m.y / n * BIGMAP - 7.0));
            text.add_theme_constant_override("outline_size", 4);
            text.add_theme_color_override("font_outline_color", Color::from_rgb(0.0, 0.0, 0.0));
            text.set_mouse_filter(MouseFilter::IGNORE);
            self.bigmap_rect.add_child(&text);
            self.bigmap_dots.push(big.upcast());
            self.bigmap_dots.push(text.upcast());
        }
        // Town names on the world map.
        for t in &s.towns {
            let mut text = label(&t.name, 13);
            text.set_position(Vector2::new(
                t.x / n * BIGMAP - 30.0,
                t.y / n * BIGMAP - 8.0,
            ));
            text.add_theme_constant_override("outline_size", 6);
            text.add_theme_color_override("font_outline_color", Color::from_rgb(0.0, 0.0, 0.0));
            text.set_mouse_filter(MouseFilter::IGNORE);
            self.bigmap_rect.add_child(&text);
            self.bigmap_dots.push(text.upcast());
        }
        self.shown_marks = s.you.marks.clone();
    }

    /// Refreshes all widgets from the latest server state.
    pub fn update(&mut self, s: &Session, lib: &ContentLibrary, yaw: f32) {
        let you = &s.you;
        let title = if you.title.is_empty() {
            String::new()
        } else {
            format!("  <{}>", you.title)
        };
        self.name
            .set_text(&format!("{}  ·  Level {}{}", s.my_name, you.level, title));
        self.hp_label
            .set_text(&format!("Health {} / {}", you.hp, you.max_hp));
        self.hp.set_max(you.max_hp.max(1.0) as f64);
        self.hp.set_value(you.hp as f64);
        self.mana_label.set_text(&format!(
            "Mana {} / {}   ·   XP {} / {}",
            you.mana, you.max_mana, you.xp, you.xp_next
        ));
        self.mana.set_max(you.max_mana.max(1.0) as f64);
        self.mana.set_value(you.mana as f64);
        self.xp.set_max(you.xp_next.max(1) as f64);
        self.xp.set_value(you.xp as f64);
        self.mood.set_max(100.0);
        self.mood.set_value(you.mood as f64);
        let points = if you.stat_points + you.perk_points > 0 {
            format!(
                "\n★ {} attribute / {} perk points — press K",
                you.stat_points, you.perk_points
            )
        } else {
            String::new()
        };
        self.stats.set_text(&format!(
            "ATK {}  DEF {}  SP {}  Crit {}%  Mood {}\nGold {}  Kills {}  Online {}{}",
            you.attack,
            you.defense,
            you.sp,
            you.crit,
            you.mood as i32,
            you.gold,
            you.kills,
            you.online,
            points
        ));
        let st: Vec<String> = you
            .statuses
            .iter()
            .map(|x| {
                let t = if x.t > 0.0 {
                    format!(" {}s", x.t as i32)
                } else {
                    String::new()
                };
                format!("[color={}]■ {}{}[/color]", x.color, x.name, t)
            })
            .collect();
        self.statuses.set_text(&st.join("   "));
        self.zone.set_text(&you.zone);
        let hours = (you.tod * 24.0) as i32;
        let mins = ((you.tod * 24.0 - hours as f32) * 60.0) as i32;
        let phase = if (0.25..0.75).contains(&you.tod) {
            "☀"
        } else {
            "☾"
        };
        self.clock
            .set_text(&format!("{phase} {hours:02}:{mins:02}"));

        // Minimap follows the player, centred.
        let n = s.size.max(1) as f32;
        let scale = MINIMAP / MINIMAP_SPAN;
        self.minimap.set_size(Vector2::new(n * scale, n * scale));
        self.minimap.set_position(Vector2::new(
            MINIMAP / 2.0 - you.x * scale,
            MINIMAP / 2.0 - you.y * scale,
        ));
        self.marker.set_rotation(-yaw);
        self.update_marks(s);
        for (dot, m) in self.mark_dots.iter_mut().zip(s.you.marks.iter()) {
            dot.set_position(Vector2::new(m.x * scale - 3.0, m.y * scale - 3.0));
        }
        if self.bigmap.is_visible() {
            self.bigmap_marker.set_position(Vector2::new(
                you.x / n * BIGMAP - 4.0,
                you.y / n * BIGMAP - 4.0,
            ));
        }
        let _ = &self.minimap_tex;

        // Spell bar.
        for (i, b) in self.spell_buttons.iter_mut().enumerate() {
            match you.spells.get(i).filter(|sp| !sp.id.is_empty()) {
                Some(sp) => {
                    let cd = if sp.cd > 0.05 {
                        format!(" {:.1}", sp.cd)
                    } else {
                        String::new()
                    };
                    b.set_text(&format!("{} {}{}", SPELL_KEYS[i], sp.name, cd));
                    let ready = sp.cd <= 0.05 && you.mana >= sp.mana;
                    let c = parse_color(&sp.color);
                    b.add_theme_color_override(
                        "font_color",
                        if ready { c } else { c.darkened(0.55) },
                    );
                    b.set_tooltip_text(&format!("{} — {} mana", sp.name, sp.mana));
                }
                None => {
                    b.set_text(SPELL_KEYS[i]);
                    b.add_theme_color_override("font_color", Color::from_rgba(1.0, 1.0, 1.0, 0.35));
                }
            }
        }

        if s.inv_version != self.shown_version {
            self.shown_version = s.inv_version;
            self.inventory.clear();
            for slot in &mut self.slots {
                slot.set_disabled(true);
                slot.set_button_icon(Gd::<godot::classes::Texture2D>::null_arg());
                slot.set_tooltip_text("");
            }
            for (i, st) in s.inventory.iter().enumerate() {
                let name = s.stack_name(st);
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
                let (kind, color, tip) = match &st.g {
                    Some(g) => (
                        if g.slot == "weapon" {
                            "weapon"
                        } else {
                            "armor"
                        },
                        parse_color(&g.color),
                        g.lines.join("\n"),
                    ),
                    None => {
                        let def = s.catalog.item(&st.item);
                        (
                            def.map(|d| d.kind.as_str()).unwrap_or("material"),
                            def.map(|d| parse_color(&d.color)).unwrap_or(Color::WHITE),
                            def.map(|d| d.desc.clone()).unwrap_or_default(),
                        )
                    }
                };
                let icon_key = match &st.g {
                    Some(g) => format!("g:{}:{}", g.base, g.color),
                    None => st.item.clone(),
                };
                let icon = lib.icon(&icon_key, kind, color);
                if let Some(slot) = self.slots.get_mut(i) {
                    slot.set_disabled(false);
                    slot.set_button_icon(&icon);
                    slot.set_tooltip_text(&format!("{text}\n{tip}"));
                }
                self.inventory.add_item_ex(&text).icon(&icon).done();
                if st.g.is_some() {
                    self.inventory.set_item_custom_fg_color(i as i32, color);
                }
                self.inventory.set_item_tooltip(i as i32, &tip);
            }
            self.equipment.clear();
            for (i, slot) in SLOTS.iter().enumerate() {
                let (text, color, tip) = match s.equip.get(*slot) {
                    Some(st) => match &st.g {
                        Some(g) => (g.name.clone(), parse_color(&g.color), g.lines.join("\n")),
                        None => (s.catalog.item_name(&st.item), Color::WHITE, String::new()),
                    },
                    None => (
                        "—".to_string(),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.4),
                        String::new(),
                    ),
                };
                let label = format!("{:<8} {}", slot, text);
                self.equipment.add_item(&label);
                self.equipment.set_item_custom_fg_color(i as i32, color);
                if !tip.is_empty() {
                    self.equipment
                        .set_item_tooltip(i as i32, &(tip + "\n(double-click to unequip)"));
                }
            }
        }
    }
}
