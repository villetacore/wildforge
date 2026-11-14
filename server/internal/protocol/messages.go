// Package protocol defines the JSON messages exchanged with game clients
// over the WebSocket. Every message has a "t" field naming its type.
package protocol

import (
	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/world"
)

// ---- client -> server

// Auth is the first message on a connection: t = "register" or "login".
type Auth struct {
	T        string `json:"t"`
	Username string `json:"username"`
	Password string `json:"password"`
}

// ClientMsg is any in-game message. Types: move{dx,dy}, attack, gather
// (interact), craft{recipe}, use{index}, drop{index}, potion, unequip{slot},
// chat{text}, cast{spell}, target{target}, talk{npc,opt,text}, menu{menu},
// tame, close.
type ClientMsg struct {
	T      string  `json:"t"`
	DX     float64 `json:"dx"`
	DY     float64 `json:"dy"`
	Text   string  `json:"text"`
	Index  int     `json:"index"`
	Slot   string  `json:"slot"`
	Recipe string  `json:"recipe"`
	Spell  int     `json:"spell"`
	Target int64   `json:"target"`
	NPC    int64   `json:"npc"`
	Opt    string  `json:"opt"`
	Menu   string  `json:"menu"`
}

// ---- server -> client

type Error struct {
	T       string `json:"t"` // "error"
	Message string `json:"message"`
}

func NewError(msg string) Error { return Error{"error", msg} }

type Registered struct {
	T        string `json:"t"` // "registered"
	Username string `json:"username"`
}

// Look is a character's customisable appearance.
type Look struct {
	Skin      int  `json:"skin"`
	Hair      int  `json:"hair"`
	HairColor int  `json:"hair_color"`
	Beard     int  `json:"beard"`
	Build     int  `json:"build"`
	Eyes      int  `json:"eyes"`
	Female    bool `json:"female"`
	Done      bool `json:"done"`
	Model     int  `json:"model"` // 0-4 skeletal hero models, 5 classic procedural
}

type NPC struct {
	ID      int64   `json:"id"`
	Name    string  `json:"name"`
	Title   string  `json:"title"`
	Role    string  `json:"role"`
	Outfit  string  `json:"outfit"`
	Color   string  `json:"color"`
	Marker  string  `json:"marker"`
	Faction string  `json:"faction"`
	X       float64 `json:"x"`
	Y       float64 `json:"y"`
	Yaw     float64 `json:"yaw"`
	Look    Look    `json:"look"`
}

// Welcome is sent once after login. Terrain, Nodes and Zones are strings with
// one character per tile ('0' + code), row-major, Size x Size.
type Welcome struct {
	T          string             `json:"t"` // "welcome"
	ID         int64              `json:"id"`
	Name       string             `json:"name"`
	Size       int                `json:"size"`
	Heights    []float64          `json:"heights"`
	Portals    []world.PortalLink `json:"portals"`
	Terrain    string             `json:"terrain"`
	Nodes      string             `json:"nodes"`
	Zones      string             `json:"zones"`
	Depleted   []int              `json:"depleted"`
	Structures []world.Structure  `json:"structures"`
	Towns      []world.TownInfo   `json:"towns"`
	Dungeons   []*world.Dungeon   `json:"dungeons"`
	NPCs       []NPC              `json:"npcs"`
	Content    *content.Pack      `json:"content"`
}

// DungeonPatch replaces a dungeon's terrain after it regenerates.
type DungeonPatch struct {
	T       string             `json:"t"` // "dungeon"
	Index   int                `json:"index"`
	X       int                `json:"x"`
	Y       int                `json:"y"`
	Size    int                `json:"size"`
	Terrain string             `json:"terrain"`
	Portals []world.PortalLink `json:"portals"`
}

type Chat struct {
	T    string `json:"t"` // "chat"
	From string `json:"from"`
	Text string `json:"text"`
}

func System(text string) Chat { return Chat{"chat", "", text} }

type Died struct {
	T        string `json:"t"` // "died"
	LostGold int    `json:"lost_gold"`
}

// Node reports a resource node being depleted or respawning.
type Node struct {
	T     string `json:"t"` // "node"
	Index int    `json:"i"`
	Alive bool   `json:"alive"`
}

// Option is one clickable line in a dialog.
type Option struct {
	ID       string `json:"id"`
	Label    string `json:"label"`
	Hint     string `json:"hint,omitempty"`
	Color    string `json:"color,omitempty"`
	Disabled bool   `json:"disabled,omitempty"`
}

// Dialog is a server-driven window: NPC conversations, shops, menus.
// NPC = 0 for menus that are not tied to an NPC.
type Dialog struct {
	T       string   `json:"t"` // "dialog"
	NPC     int64    `json:"npc"`
	Menu    string   `json:"menu"`
	Title   string   `json:"title"`
	Sub     string   `json:"sub"`
	Text    string   `json:"text"`
	Options []Option `json:"options"`
	Prompt  string   `json:"prompt,omitempty"`
}

type DialogClose struct {
	T string `json:"t"` // "dialog_close"
}

type StatusView struct {
	ID    string  `json:"id"`
	Name  string  `json:"name"`
	Color string  `json:"color"`
	T     float64 `json:"t"`
	Bad   bool    `json:"bad"`
}

type SpellSlot struct {
	ID    string  `json:"id"`
	Name  string  `json:"name"`
	Color string  `json:"color"`
	CD    float64 `json:"cd"`
	MaxCD float64 `json:"max_cd"`
	Mana  float64 `json:"mana"`
}

type Mark struct {
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	Label string  `json:"label"`
	Color string  `json:"color"`
}

type You struct {
	ID         int64                     `json:"id"`
	X          float64                   `json:"x"`
	Y          float64                   `json:"y"`
	HP         float64                   `json:"hp"`
	MaxHP      float64                   `json:"max_hp"`
	Mana       float64                   `json:"mana"`
	MaxMana    float64                   `json:"max_mana"`
	Level      int                       `json:"level"`
	XP         int                       `json:"xp"`
	XPNext     int                       `json:"xp_next"`
	Gold       int                       `json:"gold"`
	Attack     int                       `json:"attack"`
	Defense    int                       `json:"defense"`
	SP         int                       `json:"sp"`
	Crit       float64                   `json:"crit"`
	Attrs      [4]int                    `json:"attrs"`
	StatPoints int                       `json:"stat_points"`
	PerkPoints int                       `json:"perk_points"`
	Mood       float64                   `json:"mood"`
	Inventory  *[]content.Stack          `json:"inventory,omitempty"` // only sent when changed
	Equip      map[string]*content.Stack `json:"equip,omitempty"`     // only sent when changed
	Weapon     string                    `json:"weapon"`
	Armor      string                    `json:"armor"`
	Kills      int                       `json:"kills"`
	Online     int                       `json:"online"`
	Statuses   []StatusView              `json:"statuses"`
	Spells     []SpellSlot               `json:"spells"`
	Zone       string                    `json:"zone"`
	Target     int64                     `json:"target"`
	Marks      []Mark                    `json:"marks"`
	Title      string                    `json:"title"`
	TOD        float64                   `json:"tod"`
	Stamina    float64                   `json:"stamina"`
	MaxStamina float64                   `json:"max_stamina"`
	Combo      int                       `json:"combo"`
}

type Player struct {
	ID     int64   `json:"id"`
	Name   string  `json:"name"`
	Title  string  `json:"title"`
	X      float64 `json:"x"`
	Y      float64 `json:"y"`
	HP     float64 `json:"hp"`
	MaxHP  float64 `json:"max_hp"`
	Level  int     `json:"level"`
	Weapon string  `json:"weapon"` // weapon model: sword, axe, mace, staff...
	WColor string  `json:"wcolor"`
	Armor  string  `json:"armor"` // body color
	Helm   string  `json:"helm"`
	Shield string  `json:"shield"`
	Look   Look    `json:"look"`
	Anim   string  `json:"anim"`
	Seq    int     `json:"seq"`
	Yaw    float64 `json:"yaw"`
	Air    bool    `json:"air"`
}

type Enemy struct {
	ID     int64    `json:"id"`
	Kind   string   `json:"kind"`
	Name   string   `json:"name"`
	Model  string   `json:"model"`
	Tex    string   `json:"tex"`
	Color  string   `json:"color"`
	Glow   string   `json:"glow"`
	Scale  float64  `json:"scale"`
	Rank   int      `json:"rank"`
	X      float64  `json:"x"`
	Y      float64  `json:"y"`
	HP     float64  `json:"hp"`
	MaxHP  float64  `json:"max_hp"`
	Level  int      `json:"level"`
	Status []string `json:"status"`
	Anim   string   `json:"anim"`
	Seq    int      `json:"seq"`
}

type Pet struct {
	ID    int64   `json:"id"`
	Owner int64   `json:"owner"`
	Name  string  `json:"name"`
	Model string  `json:"model"`
	Tex   string  `json:"tex"`
	Color string  `json:"color"`
	Scale float64 `json:"scale"`
	Fly   bool    `json:"fly"`
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	HP    float64 `json:"hp"`
	MaxHP float64 `json:"max_hp"`
	Level int     `json:"level"`
}

type GroundItem struct {
	ID    int64   `json:"id"`
	Item  string  `json:"item"`
	Color string  `json:"color,omitempty"`
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
}

// Fx is a one-shot visual effect: hit, hurt, heal, xp, gold, loot, level,
// swing, gather, bolt (x,y -> x2,y2), nova (radius r), cast, status.
type Fx struct {
	Kind  string  `json:"kind"`
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	X2    float64 `json:"x2,omitempty"`
	Y2    float64 `json:"y2,omitempty"`
	R     float64 `json:"r,omitempty"`
	Color string  `json:"color,omitempty"`
	Text  string  `json:"text"`
}

// State is sent every tick with everything near the player.
type State struct {
	T       string       `json:"t"` // "state"
	You     You          `json:"you"`
	Players []Player     `json:"players"`
	Enemies []Enemy      `json:"enemies"`
	Pets    []Pet        `json:"pets"`
	Items   []GroundItem `json:"items"`
	Fx      []Fx         `json:"fx"`
}
