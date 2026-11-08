//! Plain data for one logged-in game session (no Godot nodes here).

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

use crate::content::parse_color;
use crate::net::protocol::{RecipeDef, You};
use crate::net::Catalog;
use crate::world::terrain::terrain_color;

pub struct Session {
    pub my_id: i64,
    pub my_name: String,
    pub catalog: Catalog,
    pub size: usize,
    pub terrain: Vec<u8>,
    pub nodes: Vec<u8>,
    pub you: You,
}

fn decode(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b.saturating_sub(b'0')).collect()
}

impl Session {
    pub fn new(
        my_id: i64,
        my_name: String,
        catalog: Catalog,
        size: usize,
        terrain: &str,
        nodes: &str,
    ) -> Session {
        Session {
            my_id,
            my_name,
            catalog,
            size,
            terrain: decode(terrain),
            nodes: decode(nodes),
            you: You::default(),
        }
    }

    pub fn count(&self, item: &str) -> i32 {
        self.you
            .inventory
            .iter()
            .filter(|s| s.item == item)
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

    /// Minimap: terrain colors with resource nodes as darker dots.
    pub fn minimap_texture(&self) -> Option<Gd<ImageTexture>> {
        let n = self.size;
        let mut data = PackedByteArray::new();
        data.resize(n * n * 3);
        let bytes = data.as_mut_slice();
        for i in 0..n * n {
            let mut c = terrain_color(self.terrain[i]);
            if let Some(r) = self.catalog.resource(self.nodes[i]) {
                c = if r.model == "tree" {
                    Color::from_rgb(0.12, 0.3, 0.12)
                } else {
                    parse_color(&r.color).darkened(0.2)
                };
            }
            bytes[i * 3] = (c.r * 255.0) as u8;
            bytes[i * 3 + 1] = (c.g * 255.0) as u8;
            bytes[i * 3 + 2] = (c.b * 255.0) as u8;
        }
        let img = Image::create_from_data(n as i32, n as i32, false, Format::RGB8, &data)?;
        ImageTexture::create_from_image(&img)
    }
}
