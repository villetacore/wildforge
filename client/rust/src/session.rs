//! Plain data for one logged-in game session (no Godot nodes here).

use std::collections::HashMap;

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

use crate::content::parse_color;
use crate::net::protocol::{
    DungeonInfo, NpcInfo, PortalLink, RecipeDef, Stack, Structure, TownInfo, WelcomeMsg, You,
};
use crate::net::Catalog;
use crate::world::terrain::terrain_color;

pub struct Session {
    pub my_id: i64,
    pub my_name: String,
    pub catalog: Catalog,
    pub size: usize,
    pub terrain: Vec<u8>,
    pub heights: Vec<f32>,
    pub nodes: Vec<u8>,
    pub zones: Vec<u8>,
    pub structures: Vec<Structure>,
    pub towns: Vec<TownInfo>,
    pub dungeons: Vec<DungeonInfo>,
    pub portals: Vec<PortalLink>,
    pub npcs: Vec<NpcInfo>,
    pub you: You,
    pub inventory: Vec<Stack>,
    pub equip: HashMap<String, Stack>,
    pub inv_version: u64,
}

fn decode(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b.saturating_sub(b'0')).collect()
}

impl Session {
    pub fn new(w: WelcomeMsg) -> Session {
        Session {
            my_id: w.id,
            my_name: w.name,
            catalog: Catalog::new(w.content),
            size: w.size,
            terrain: decode(&w.terrain),
            heights: w.heights,
            nodes: decode(&w.nodes),
            zones: decode(&w.zones),
            structures: w.structures,
            towns: w.towns,
            dungeons: w.dungeons,
            portals: w.portals,
            npcs: w.npcs,
            you: You::default(),
            inventory: Vec::new(),
            equip: HashMap::new(),
            inv_version: 0,
        }
    }

    /// Takes a state update; inventory and equipment only arrive when changed.
    pub fn set_you(&mut self, mut you: You) {
        if let Some(inv) = you.inventory.take() {
            self.inventory = inv;
            self.equip = you.equip.take().unwrap_or_default();
            self.inv_version += 1;
        }
        self.you = you;
    }

    pub fn zone_at(&self, x: f32, y: f32) -> Option<u8> {
        if x < 0.0 || y < 0.0 || x >= self.size as f32 || y >= self.size as f32 {
            return None;
        }
        self.zones.get(y as usize * self.size + x as usize).copied()
    }

    /// Writes a dungeon's regenerated terrain into the map.
    pub fn apply_dungeon(
        &mut self,
        x: usize,
        y: usize,
        size: usize,
        terrain: &str,
        portals: Vec<PortalLink>,
    ) {
        let cells = decode(terrain);
        for row in 0..size {
            for col in 0..size {
                let i = (y + row) * self.size + x + col;
                if let (Some(dst), Some(src)) =
                    (self.terrain.get_mut(i), cells.get(row * size + col))
                {
                    *dst = *src;
                }
            }
        }
        self.portals = portals;
    }

    pub fn count(&self, item: &str) -> i32 {
        self.inventory
            .iter()
            .filter(|s| s.item == item && s.g.is_none())
            .map(|s| s.qty)
            .sum()
    }

    pub fn can_craft(&self, r: &RecipeDef) -> bool {
        r.ingredients.iter().all(|(id, n)| self.count(id) >= *n)
    }

    /// "Wood 3/5, Stone 0/3"
    pub fn ingredients_text(&self, r: &RecipeDef) -> String {
        r.ingredients
            .iter()
            .map(|(id, n)| {
                format!(
                    "{} {}/{}",
                    self.catalog.item_name(id),
                    self.count(id).min(*n),
                    n
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn stack_name(&self, s: &Stack) -> String {
        match &s.g {
            Some(g) => g.name.clone(),
            None => self.catalog.item_name(&s.item),
        }
    }

    /// Minimap: terrain colors, zone tint, resource dots and buildings.
    pub fn minimap_texture(&self) -> Option<Gd<ImageTexture>> {
        let n = self.size;
        let mut data = PackedByteArray::new();
        data.resize(n * n * 3);
        let bytes = data.as_mut_slice();
        let zone_colors: Vec<Color> = self
            .catalog
            .defs
            .zones
            .iter()
            .map(|z| parse_color(&z.color))
            .collect();
        for i in 0..n * n {
            let code = self.terrain[i];
            let mut c = terrain_color(code);
            if let Some(zc) = self.zones.get(i).and_then(|z| zone_colors.get(*z as usize)) {
                if !matches!(code, 1 | 4 | 6..=9 | 13..=17) {
                    c = c.lerp(*zc, 0.25);
                }
            }
            if let Some(r) = self.catalog.resource(self.nodes[i]) {
                c = if matches!(r.model.as_str(), "tree" | "pine" | "palm" | "deadtree") {
                    c.darkened(0.3)
                } else {
                    parse_color(&r.color).darkened(0.2)
                };
            }
            let (x, y) = (i % n, i / n);
            if self
                .dungeons
                .iter()
                .any(|d| x >= d.x && y >= d.y && x < d.x + d.size && y < d.y + d.size)
            {
                c = Color::from_rgb(0.1, 0.25, 0.45); // dungeons are hidden under the sea
            }
            bytes[i * 3] = (c.r.clamp(0.0, 1.0) * 255.0) as u8;
            bytes[i * 3 + 1] = (c.g.clamp(0.0, 1.0) * 255.0) as u8;
            bytes[i * 3 + 2] = (c.b.clamp(0.0, 1.0) * 255.0) as u8;
        }
        let img = Image::create_from_data(n as i32, n as i32, false, Format::RGB8, &data)?;
        ImageTexture::create_from_image(&img)
    }
}
