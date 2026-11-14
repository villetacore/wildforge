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
    pub price: i32,
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
pub struct SpellDef {
    pub id: String,
    pub name: String,
    pub school: String,
    pub color: String,
    pub desc: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ZoneDef {
    pub id: String,
    pub name: String,
    pub biome: String,
    pub fog: String,
    pub ambient: String,
    pub color: String,
    pub min_level: i32,
    pub max_level: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ContentDefs {
    pub items: Vec<ItemDef>,
    pub resources: Vec<ResourceDef>,
    pub recipes: Vec<RecipeDef>,
    pub spells: Vec<SpellDef>,
    pub zones: Vec<ZoneDef>,
    pub factions: Vec<FactionDef>,
}

/// Content definitions with lookup indexes.
pub struct Catalog {
    pub defs: ContentDefs,
    items: HashMap<String, usize>,
}

impl Catalog {
    pub fn new(defs: ContentDefs) -> Catalog {
        let items = defs
            .items
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id.clone(), i))
            .collect();
        Catalog { defs, items }
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
    /// Resource for a node code from the `nodes` map string (1-based).
    pub fn resource(&self, code: u8) -> Option<&ResourceDef> {
        (code as usize)
            .checked_sub(1)
            .and_then(|i| self.defs.resources.get(i))
    }
    pub fn zone(&self, i: u8) -> Option<&ZoneDef> {
        self.defs.zones.get(i as usize)
    }
}

// ---- world layout (sent once in `welcome`)

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Structure {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub kind: String,
    pub style: String,
    pub floors: i32,
    pub wall: String,
    pub roof: String,
    pub name: String,
    pub solid: bool,
    pub door: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct TownInfo {
    pub name: String,
    pub zone: i32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub style: String,
    pub faction: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DungeonInfo {
    pub index: usize,
    pub zone: usize,
    pub name: String,
    pub theme: String,
    pub x: usize,
    pub y: usize,
    pub size: usize,
    pub surface_x: f32,
    pub surface_y: f32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct PortalLink {
    pub x: f32,
    pub y: f32,
    pub to_x: f32,
    pub to_y: f32,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Look {
    pub skin: i32,
    pub hair: i32,
    pub hair_color: i32,
    pub beard: i32,
    pub build: i32,
    pub eyes: i32,
    pub female: bool,
    pub done: bool,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct NpcInfo {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub role: String,
    pub outfit: String,
    pub color: String,
    pub marker: String,
    pub faction: String,
    pub x: f32,
    pub y: f32,
    pub yaw: f32,
    pub look: Look,
}

// ---- per-tick state

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Gear {
    pub base: String,
    pub name: String,
    pub slot: String,
    pub model: String,
    pub color: String,
    pub tint: String,
    pub rarity: i32,
    pub value: i32,
    pub lines: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Stack {
    pub item: String,
    pub qty: i32,
    pub g: Option<Gear>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct StatusView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub t: f32,
    pub bad: bool,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct SpellSlot {
    pub id: String,
    pub name: String,
    pub color: String,
    pub cd: f32,
    pub max_cd: f32,
    pub mana: f32,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Mark {
    pub x: f32,
    pub y: f32,
    pub label: String,
    pub color: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct You {
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub level: i32,
    pub xp: i32,
    pub xp_next: i32,
    pub gold: i32,
    pub attack: i32,
    pub defense: i32,
    pub sp: i32,
    pub crit: f32,
    pub attrs: [i32; 4],
    pub stat_points: i32,
    pub perk_points: i32,
    pub mood: f32,
    pub inventory: Option<Vec<Stack>>,
    pub equip: Option<HashMap<String, Stack>>,
    pub weapon: String,
    pub armor: String,
    pub kills: i32,
    pub online: i32,
    pub statuses: Vec<StatusView>,
    pub spells: Vec<SpellSlot>,
    pub zone: String,
    pub target: i64,
    pub marks: Vec<Mark>,
    pub title: String,
    pub tod: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub combo: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct PlayerState {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
    pub weapon: String,
    pub wcolor: String,
    pub armor: String,
    pub helm: String,
    pub shield: String,
    pub look: Look,
    pub anim: String,
    pub seq: i64,
    pub yaw: f32,
    pub air: bool,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct EnemyState {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub model: String,
    pub tex: String,
    pub color: String,
    pub glow: String,
    pub scale: f32,
    pub rank: i32,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
    pub status: Vec<String>,
    pub anim: String,
    pub seq: i64,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct PetState {
    pub id: i64,
    pub owner: i64,
    pub name: String,
    pub model: String,
    pub tex: String,
    pub color: String,
    pub scale: f32,
    pub fly: bool,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub level: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ItemState {
    pub id: i64,
    pub item: String,
    pub color: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Fx {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub x2: f32,
    pub y2: f32,
    pub r: f32,
    pub color: String,
    pub text: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DialogOption {
    pub id: String,
    pub label: String,
    pub hint: String,
    pub color: String,
    pub disabled: bool,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Dialog {
    pub npc: i64,
    pub menu: String,
    pub title: String,
    pub sub: String,
    pub text: String,
    pub options: Vec<DialogOption>,
    pub prompt: String,
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
    Welcome(Box<WelcomeMsg>),
    State {
        you: Box<You>,
        players: Vec<PlayerState>,
        enemies: Vec<EnemyState>,
        #[serde(default)]
        pets: Vec<PetState>,
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
    Dialog(Box<Dialog>),
    DialogClose,
    Dungeon {
        index: usize,
        x: usize,
        y: usize,
        size: usize,
        terrain: String,
        portals: Vec<PortalLink>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct WelcomeMsg {
    pub id: i64,
    pub name: String,
    pub size: usize,
    pub terrain: String,
    pub heights: Vec<f32>,
    pub nodes: String,
    pub zones: String,
    pub depleted: Vec<usize>,
    pub portals: Vec<PortalLink>,
    pub structures: Vec<Structure>,
    pub towns: Vec<TownInfo>,
    pub dungeons: Vec<DungeonInfo>,
    pub npcs: Vec<NpcInfo>,
    pub content: ContentDefs,
}
