//! Crafting window: lists recipes from the content pack and shows which
//! ones can be made with the current inventory.

use godot::classes::control::FocusMode;
use godot::classes::text_server::AutowrapMode;
use godot::classes::{CanvasLayer, ItemList, Label, PanelContainer, VBoxContainer};
use godot::prelude::*;

use super::{button, label, panel, push, UiEvent, UiQueue, SCREEN_W};
use crate::content::{parse_color, ContentLibrary};
use crate::session::Session;

pub struct CraftingPanel {
    root: Gd<PanelContainer>,
    list: Gd<ItemList>,
    details: Gd<Label>,
    recipes: Vec<String>,
    signature: String,
}

impl CraftingPanel {
    pub fn build(layer: &mut Gd<CanvasLayer>, q: &UiQueue) -> CraftingPanel {
        let mut root = panel(
            Vector2::new(SCREEN_W - 560.0, 60.0),
            Vector2::new(320.0, 0.0),
            0.9,
        );
        let mut v = VBoxContainer::new_alloc();
        v.add_child(&label("Crafting", 18));
        let mut hint = label("double-click a recipe to craft · C to close", 11);
        hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6));
        v.add_child(&hint);
        let mut list = ItemList::new_alloc();
        list.set_custom_minimum_size(Vector2::new(0.0, 330.0));
        list.set_focus_mode(FocusMode::NONE);
        list.add_theme_font_size_override("font_size", 13);
        v.add_child(&list);
        let mut details = label("", 12);
        details.set_autowrap_mode(AutowrapMode::WORD);
        details.set_custom_minimum_size(Vector2::new(0.0, 48.0));
        v.add_child(&details);
        let craft = button("Craft");
        v.add_child(&craft);
        root.add_child(&v);
        root.set_visible(false);
        layer.add_child(&root);

        let qc = q.clone();
        craft
            .signals()
            .pressed()
            .connect(move || push(&qc, UiEvent::CraftSelected));
        CraftingPanel {
            root,
            list,
            details,
            recipes: Vec::new(),
            signature: String::new(),
        }
    }

    /// Connects activation after construction (needs the recipe ids).
    pub fn connect(&self, q: &UiQueue, s: &Session) {
        let ids: Vec<String> = s
            .catalog
            .defs
            .recipes
            .iter()
            .map(|r| r.id.clone())
            .collect();
        let qc = q.clone();
        self.list.signals().item_activated().connect(move |i: i64| {
            if let Some(id) = ids.get(i as usize) {
                push(&qc, UiEvent::Craft(id.clone()));
            }
        });
    }

    pub fn toggle(&mut self) {
        let v = !self.root.is_visible();
        self.root.set_visible(v);
        self.signature.clear(); // force refresh
    }

    pub fn selected(&self) -> Option<String> {
        let sel = self.list.get_selected_items();
        sel.as_slice()
            .first()
            .and_then(|i| self.recipes.get(*i as usize).cloned())
    }

    pub fn update(&mut self, s: &Session, lib: &ContentLibrary) {
        if !self.root.is_visible() {
            return;
        }
        let sig = s.inv_version.to_string();
        if sig != self.signature {
            self.signature = sig;
            let selected = self.list.get_selected_items().as_slice().first().copied();
            self.list.clear();
            self.recipes.clear();
            for (i, r) in s.catalog.defs.recipes.iter().enumerate() {
                let def = s.catalog.item(&r.result);
                let name = s.catalog.item_name(&r.result);
                let qty = if r.qty > 1 {
                    format!(" ×{}", r.qty)
                } else {
                    String::new()
                };
                let (kind, color) = def
                    .map(|d| (d.kind.as_str(), parse_color(&d.color)))
                    .unwrap_or(("material", Color::WHITE));
                self.list
                    .add_item_ex(&format!("{name}{qty}"))
                    .icon(&lib.icon(&r.result, kind, color))
                    .done();
                let ok = s.can_craft(r);
                let c = if ok {
                    Color::from_rgb(0.65, 1.0, 0.6)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                };
                self.list.set_item_custom_fg_color(i as i32, c);
                let tip = format!(
                    "{}\n{}",
                    def.map(|d| d.desc.as_str()).unwrap_or(""),
                    s.ingredients_text(r)
                );
                self.list.set_item_tooltip(i as i32, &tip);
                self.recipes.push(r.id.clone());
            }
            if let Some(i) = selected {
                if (i as usize) < self.recipes.len() {
                    self.list.select(i);
                }
            }
        }
        let text = match self.list.get_selected_items().as_slice().first() {
            Some(&i) => {
                let r = &s.catalog.defs.recipes[i as usize];
                let desc = s
                    .catalog
                    .item(&r.result)
                    .map(|d| d.desc.clone())
                    .unwrap_or_default();
                format!("{desc}\nNeeds: {}", s.ingredients_text(r))
            }
            None => "Select a recipe to see what it needs.".into(),
        };
        self.details.set_text(&text);
    }
}
