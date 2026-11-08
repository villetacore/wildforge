// Package protocol defines the JSON messages exchanged with game clients
// over the WebSocket. Every message has a "t" field naming its type.
package protocol

import "github.com/villetacore/wildforge/server/internal/content"

// ---- client -> server

// Auth is the first message on a connection: t = "register" or "login".
type Auth struct {
	T        string `json:"t"`
	Username string `json:"username"`
	Password string `json:"password"`
}

// ClientMsg is any in-game message. Types: move{dx,dy}, attack, gather,
// craft{recipe}, use{index}, drop{index}, potion, unequip{slot}, chat{text}.
type ClientMsg struct {
	T      string  `json:"t"`
	DX     float64 `json:"dx"`
	DY     float64 `json:"dy"`
	Text   string  `json:"text"`
	Index  int     `json:"index"`
	Slot   string  `json:"slot"`
	Recipe string  `json:"recipe"`
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

// Welcome is sent once after login. Terrain and Nodes are strings with one
// character per tile ('0' + code), row-major, Size x Size.
type Welcome struct {
	T        string        `json:"t"` // "welcome"
	ID       int64         `json:"id"`
	Name     string        `json:"name"`
	Size     int           `json:"size"`
	Terrain  string        `json:"terrain"`
	Nodes    string        `json:"nodes"`
	Depleted []int         `json:"depleted"`
	Content  *content.Pack `json:"content"`
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

type You struct {
	ID        int64           `json:"id"`
	X         float64         `json:"x"`
	Y         float64         `json:"y"`
	HP        float64         `json:"hp"`
	MaxHP     float64         `json:"max_hp"`
	Level     int             `json:"level"`
	XP        int             `json:"xp"`
	XPNext    int             `json:"xp_next"`
	Gold      int             `json:"gold"`
	Attack    int             `json:"attack"`
	Defense   int             `json:"defense"`
	Inventory []content.Stack `json:"inventory"`
	Weapon    string          `json:"weapon"`
	Armor     string          `json:"armor"`
	Kills     int             `json:"kills"`
	Online    int             `json:"online"`
}

type Player struct {
	ID     int64   `json:"id"`
	Name   string  `json:"name"`
	X      float64 `json:"x"`
	Y      float64 `json:"y"`
	HP     float64 `json:"hp"`
	MaxHP  float64 `json:"max_hp"`
	Level  int     `json:"level"`
	Weapon string  `json:"weapon"`
	Armor  string  `json:"armor"`
}

type Enemy struct {
	ID    int64   `json:"id"`
	Kind  string  `json:"kind"`
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	HP    float64 `json:"hp"`
	MaxHP float64 `json:"max_hp"`
	Level int     `json:"level"`
}

type GroundItem struct {
	ID   int64   `json:"id"`
	Item string  `json:"item"`
	X    float64 `json:"x"`
	Y    float64 `json:"y"`
}

// Fx is a one-shot visual effect: hit, hurt, heal, xp, gold, loot, level,
// swing, gather.
type Fx struct {
	Kind string  `json:"kind"`
	X    float64 `json:"x"`
	Y    float64 `json:"y"`
	Text string  `json:"text"`
}

// State is sent every tick with everything near the player.
type State struct {
	T       string       `json:"t"` // "state"
	You     You          `json:"you"`
	Players []Player     `json:"players"`
	Enemies []Enemy      `json:"enemies"`
	Items   []GroundItem `json:"items"`
	Fx      []Fx         `json:"fx"`
}
