//! Wire types shared with the Go server (`server/internal/protocol`).

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

// ---- content definitions (sent once in `welcome`)

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub desc: String,
    pub color: String,
    pub stack: i32,
    pub heal: f32,
    pub attack: i32,
    pub defense: i32,
    pub tool: String,
    pub tier: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ResourceDef {
    pub id: String,
    pub name: String,
    pub item: String,
    pub tool: String,
    pub min_tier: i32,
    pub model: String,
    pub texture: String,
    pub color: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct RecipeDef {
    pub id: String,
    pub result: String,
    pub qty: i32,
    pub ingredients: BTreeMap<String, i32>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct EnemyDef {
    pub id: String,
    pub name: String,
    pub model: String,
    pub texture: String,
    pub color: String,
    pub scale: f32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ContentDefs {
    pub items: Vec<ItemDef>,
    pub resources: Vec<ResourceDef>,
    pub recipes: Vec<RecipeDef>,
    pub enemies: Vec<EnemyDef>,
}

/// Content definitions with lookup indexes.
pub struct Catalog {
    pub defs: ContentDefs,
    items: HashMap<String, usize>,
    enemies: HashMap<String, usize>,
}

impl Catalog {
    pub fn new(defs: ContentDefs) -> Catalog {
        let items = defs
            .items
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id.clone(), i))
            .collect();
        let enemies = defs
            .enemies
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id.clone(), i))
            .collect();
        Catalog {
            defs,
            items,
            enemies,
        }
    }
    pub fn item(&self, id: &str) -> Option<&ItemDef> {
        self.items.get(id).map(|i| &self.defs.items[*i])
    }
    pub fn item_name(&self, id: &str) -> String {
        if id == "gold" {
            return "Gold".into();
        }
        self.item(id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| id.to_string())
    }
    pub fn enemy(&self, id: &str) -> Option<&EnemyDef> {
        self.enemies.get(id).map(|i| &self.defs.enemies[*i])
    }
    /// Resource for a node code from the `nodes` map string (1-based).
    pub fn resource(&self, code: u8) -> Option<&ResourceDef> {
        (code as usize)
            .checked_sub(1)
            .and_then(|i| self.defs.resources.get(i))
    }
}

// ---- per-tick state

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Stack {
    pub item: String,
    pub qty: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct You {
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
    pub xp: i32,
    pub xp_next: i32,
    pub gold: i32,
    pub attack: i32,
    pub defense: i32,
    pub inventory: Vec<Stack>,
    pub weapon: String,
    pub armor: String,
    pub kills: i32,
    pub online: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerState {
    pub id: i64,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
    #[serde(default)]
    pub weapon: String,
    #[serde(default)]
    pub armor: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EnemyState {
    pub id: i64,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ItemState {
    pub id: i64,
    pub item: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Fx {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Registered {
        username: String,
    },
    Error {
        message: String,
    },
    Welcome {
        id: i64,
        name: String,
        size: usize,
        terrain: String,
        #[serde(default)]
        heights: Vec<f32>,
        nodes: String,
        depleted: Vec<usize>,
        content: ContentDefs,
    },
    State {
        you: You,
        players: Vec<PlayerState>,
        enemies: Vec<EnemyState>,
        items: Vec<ItemState>,
        fx: Vec<Fx>,
    },
    Chat {
        from: String,
        text: String,
    },
    Died {
        lost_gold: i32,
    },
    Node {
        i: usize,
        alive: bool,
    },
    #[serde(other)]
    Unknown,
}
