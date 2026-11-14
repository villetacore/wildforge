//! The server-driven dialog window: NPC conversations, shops, the smith's
//! forge, enchanting, the contract board and the player's own menus
//! (character, perks, spells, pets, reputation, contracts, appearance).

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, CanvasLayer, HBoxContainer, Label, LineEdit, PanelContainer, RichTextLabel,
    ScrollContainer, VBoxContainer,
};
use godot::prelude::*;

use super::{button, label, line_edit, panel, push, UiEvent, UiQueue, SCREEN_H};
use crate::content::parse_color;
use crate::net::protocol::Dialog;

pub struct DialogPanel {
    root: Gd<PanelContainer>,
    title: Gd<Label>,
    sub: Gd<Label>,
    text: Gd<RichTextLabel>,
    options: Gd<VBoxContainer>,
    prompt_row: Gd<HBoxContainer>,
    pub prompt: Gd<LineEdit>,
    pub npc: i64,
    pub menu: String,
    q: UiQueue,
}

impl DialogPanel {
    pub fn build(layer: &mut Gd<CanvasLayer>, q: &UiQueue) -> DialogPanel {
        let mut root = panel(Vector2::new(330.0, 70.0), Vector2::new(560.0, 0.0), 0.95);
        let mut v = VBoxContainer::new_alloc();
        v.add_theme_constant_override("separation", 6);
        let mut head = HBoxContainer::new_alloc();
        let mut titles = VBoxContainer::new_alloc();
        titles.set_h_size_flags(SizeFlags::EXPAND_FILL);
        let title = label("", 20);
        let mut sub = label("", 12);
        sub.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.65));
        titles.add_child(&title);
        titles.add_child(&sub);
        head.add_child(&titles);
        let mut close = button("✕");
        close.set_custom_minimum_size(Vector2::new(34.0, 30.0));
        let qc = q.clone();
        close
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::DialogClose));
        head.add_child(&close);
        v.add_child(&head);

        let mut text = RichTextLabel::new_alloc();
        text.set_use_bbcode(true);
        text.set_fit_content(true);
        text.set_custom_minimum_size(Vector2::new(530.0, 0.0));
        text.add_theme_font_size_override("normal_font_size", 14);
        text.set_focus_mode(FocusMode::NONE);
        text.set_selection_enabled(false);
        let mut text_scroll = ScrollContainer::new_alloc();
        text_scroll.set_custom_minimum_size(Vector2::new(530.0, 40.0));
        text_scroll.add_child(&text);
        v.add_child(&text_scroll);

        let mut scroll = ScrollContainer::new_alloc();
        scroll.set_custom_minimum_size(Vector2::new(530.0, 300.0));
        scroll.set_v_size_flags(SizeFlags::EXPAND_FILL);
        let mut options = VBoxContainer::new_alloc();
        options.set_h_size_flags(SizeFlags::EXPAND_FILL);
        options.add_theme_constant_override("separation", 3);
        scroll.add_child(&options);
        v.add_child(&scroll);

        let mut prompt_row = HBoxContainer::new_alloc();
        let mut prompt = line_edit("", "");
        prompt.set_h_size_flags(SizeFlags::EXPAND_FILL);
        prompt_row.add_child(&prompt);
        let ok = button("OK");
        prompt_row.add_child(&ok);
        prompt_row.set_visible(false);
        v.add_child(&prompt_row);
        let qc = q.clone();
        let pc = prompt.clone();
        ok.signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::DialogSubmit(pc.get_text().to_string())));
        let qc = q.clone();
        prompt
            .signals()
            .text_submitted()
            .connect(move |t: GString| push(&qc, UiEvent::DialogSubmit(t.to_string())));

        root.add_child(&v);
        root.set_visible(false);
        layer.add_child(&root);
        // Keep text area height sensible.
        let _ = SCREEN_H;
        let _ = text_scroll;
        DialogPanel {
            root,
            title,
            sub,
            text,
            options,
            prompt_row,
            prompt,
            npc: 0,
            menu: String::new(),
            q: q.clone(),
        }
    }

    pub fn visible(&self) -> bool {
        self.root.is_visible()
    }

    pub fn prompt_focused(&self) -> bool {
        self.prompt.has_focus()
    }

    pub fn show(&mut self, d: &Dialog) {
        self.npc = d.npc;
        self.menu = d.menu.clone();
        self.title.set_text(&d.title);
        self.sub.set_text(&d.sub);
        self.text.set_text(&d.text);
        let lines = d.text.lines().count().max(1) + d.text.len() / 70;
        if let Some(mut sc) = self
            .text
            .get_parent()
            .and_then(|p| p.try_cast::<ScrollContainer>().ok())
        {
            sc.set_custom_minimum_size(Vector2::new(530.0, (lines as f32 * 19.0 + 8.0).min(230.0)));
        }
        for mut c in self.options.get_children().iter_shared() {
            c.queue_free();
        }
        for o in &d.options {
            let mut b = Button::new_alloc();
            b.set_text(&o.label);
            b.set_focus_mode(FocusMode::NONE);
            b.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            b.set_autowrap_mode(AutowrapMode::WORD_SMART);
            b.set_custom_minimum_size(Vector2::new(520.0, 30.0));
            b.add_theme_font_size_override("font_size", 14);
            if !o.hint.is_empty() {
                b.set_tooltip_text(&o.hint);
            }
            if !o.color.is_empty() {
                let c = parse_color(&o.color);
                b.add_theme_color_override("font_color", c);
                b.add_theme_color_override("font_hover_color", c.lightened(0.3));
            }
            b.set_disabled(o.disabled);
            let qc = self.q.clone();
            let id = o.id.clone();
            b.signals()
                .pressed()
                .connect(move || push(&qc, UiEvent::DialogOption(id.clone())));
            self.options.add_child(&b);
        }
        let has_prompt = !d.prompt.is_empty();
        self.prompt_row.set_visible(has_prompt);
        if has_prompt {
            self.prompt.set_placeholder(&d.prompt);
            self.prompt.clear();
            self.prompt.grab_focus();
        }
        self.root.set_visible(true);
        self.root.reset_size();
    }

    pub fn hide(&mut self) {
        self.root.set_visible(false);
        self.npc = 0;
        self.menu.clear();
        self.prompt.release_focus();
    }
}

#[allow(dead_code)]
fn _unused(_: AutowrapMode) {}
