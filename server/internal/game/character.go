package game

import (
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

// Equipment slots in display order.
var slots = []string{"weapon", "offhand", "head", "chest", "legs", "hands", "feet", "ring", "amulet"}

// Attribute indices into Character.Attrs.
const (
	attrStr = iota
	attrDex
	attrInt
	attrVit
)

var attrNames = [4]string{"Strength", "Dexterity", "Intellect", "Vitality"}
var attrKeys = [4]string{"str", "dex", "int", "vit"}

// Status is an active effect on a player or enemy.
type Status struct {
	ID  string  `json:"id"`
	T   float64 `json:"t"` // seconds left; < 0 = until cured
	Mag float64 `json:"mag"`
}

type PetData struct {
	Kind    string  `json:"kind"` // pet def id, or "tamed:<family>"
	Name    string  `json:"name"`
	Level   int     `json:"level"`
	XP      int     `json:"xp"`
	HP      float64 `json:"hp"`
	Model   string  `json:"model"`
	Texture string  `json:"texture"`
	Color   string  `json:"color"`
	Scale   float64 `json:"scale"`
	Fly     bool    `json:"fly"`
	BaseHP  float64 `json:"base_hp"`
	BaseDmg float64 `json:"base_dmg"`
	Speed   float64 `json:"speed"`
	Ability string  `json:"ability"`
}

type Contract struct {
	ID      int64   `json:"id"`
	Kind    string  `json:"kind"` // hunt, cull
	Title   string  `json:"title"`
	Desc    string  `json:"desc"`
	Town    int     `json:"town"`
	Zone    int     `json:"zone"`
	Family  string  `json:"family"`
	Target  string  `json:"target"`
	X       float64 `json:"x"`
	Y       float64 `json:"y"`
	Level   int     `json:"level"`
	Need    int     `json:"need"`
	Have    int     `json:"have"`
	Gold    int     `json:"gold"`
	XP      int     `json:"xp"`
	Faction string  `json:"faction"`
	Rep     int     `json:"rep"`
	Done    bool    `json:"done"`
	Spawned bool    `json:"-"`
}

// Character is the persistent part of a player, saved as JSON by storage.
type Character struct {
	Level      int                       `json:"level"`
	XP         int                       `json:"xp"`
	HP         float64                   `json:"hp"`
	Mana       float64                   `json:"mana"`
	Gold       int                       `json:"gold"`
	X          float64                   `json:"x"`
	Y          float64                   `json:"y"`
	Inventory  []content.Stack           `json:"inventory"`
	Equip      map[string]*content.Stack `json:"equip"`
	Weapon     string                    `json:"weapon,omitempty"` // legacy, migrated into Equip
	Armor      string                    `json:"armor,omitempty"`  // legacy, migrated into Equip
	Kills      int                       `json:"kills"`
	Attrs      [4]int                    `json:"attrs"`
	StatPoints int                       `json:"stat_points"`
	PerkPoints int                       `json:"perk_points"`
	Perks      []string                  `json:"perks"`
	Spells     []string                  `json:"spells"`
	SpellBar   [6]string                 `json:"spell_bar"`
	Look       protocol.Look             `json:"look"`
	Rep        map[string]int            `json:"rep"`
	Pets       []PetData                 `json:"pets"`
	ActivePet  int                       `json:"active_pet"`
	Contracts  []Contract                `json:"contracts"`
	Statuses   []Status                  `json:"statuses"`
	Mood       float64                   `json:"mood"`
	Title      string                    `json:"title"`
	Bosses     int                       `json:"bosses"`
	Migrated   int                       `json:"migrated"`
}

func newCharacter(pack *content.Pack) *Character {
	c := &Character{Level: 1, HP: 100, Mana: 50, Gold: pack.World.StartGold, Mood: 60, ActivePet: -1}
	c.Inventory = append(c.Inventory, pack.World.StartInventory...)
	c.Spells = append(c.Spells, pack.World.StartSpells...)
	c.normalize(pack)
	return c
}

// normalize fills maps and migrates legacy saves.
func (c *Character) normalize(pack *content.Pack) {
	if c.Equip == nil {
		c.Equip = map[string]*content.Stack{}
	}
	if c.Rep == nil {
		c.Rep = map[string]int{}
	}
	if c.Weapon != "" {
		c.Equip["weapon"] = &content.Stack{Item: c.Weapon, Qty: 1}
		c.Weapon = ""
	}
	if c.Armor != "" {
		c.Equip["chest"] = &content.Stack{Item: c.Armor, Qty: 1}
		c.Armor = ""
	}
	if c.Migrated < 1 {
		// Characters from before spells, attributes and perks.
		c.Migrated = 1
		c.Spells = append(c.Spells, pack.World.StartSpells...)
		c.StatPoints += 2 * (c.Level - 1)
		c.PerkPoints += c.Level / 2
		if c.Mood == 0 {
			c.Mood = 60
		}
		c.ActivePet = -1
		c.Look.Done = c.Level > 1 // veterans keep the default look unless they visit a barber
	}
	known := map[string]bool{}
	spells := c.Spells[:0]
	for _, s := range c.Spells {
		if pack.Spell(s) != nil && !known[s] {
			known[s] = true
			spells = append(spells, s)
		}
	}
	c.Spells = spells
	empty := true
	for _, s := range c.SpellBar {
		if s != "" {
			empty = false
		}
	}
	if empty {
		for i, s := range c.Spells {
			if i < len(c.SpellBar) {
				c.SpellBar[i] = s
			}
		}
	}
	if c.ActivePet >= len(c.Pets) {
		c.ActivePet = -1
	}
}

func (c *Character) clone() *Character {
	cp := *c
	cp.Inventory = append([]content.Stack(nil), c.Inventory...)
	cp.Equip = make(map[string]*content.Stack, len(c.Equip))
	for k, v := range c.Equip {
		s := *v
		cp.Equip[k] = &s
	}
	cp.Rep = make(map[string]int, len(c.Rep))
	for k, v := range c.Rep {
		cp.Rep[k] = v
	}
	cp.Perks = append([]string(nil), c.Perks...)
	cp.Spells = append([]string(nil), c.Spells...)
	cp.Pets = append([]PetData(nil), c.Pets...)
	cp.Contracts = append([]Contract(nil), c.Contracts...)
	cp.Statuses = append([]Status(nil), c.Statuses...)
	return &cp
}

func (c *Character) hasPerk(id string) bool {
	for _, p := range c.Perks {
		if p == id {
			return true
		}
	}
	return false
}

func (c *Character) knows(spell string) bool {
	for _, s := range c.Spells {
		if s == spell {
			return true
		}
	}
	return false
}

func XPForNext(level int) int { return int(40 * math.Pow(float64(level), 1.6)) }

// ---- inventory: a list of stacks with a slot limit

type inventory struct {
	c    *Character
	pack *content.Pack
}

func (c *Character) inv(pack *content.Pack) inventory { return inventory{c, pack} }

func (v inventory) Count(item string) int {
	n := 0
	for _, s := range v.c.Inventory {
		if s.Item == item && s.G == nil {
			n += s.Qty
		}
	}
	return n
}

func (v inventory) Full() bool { return len(v.c.Inventory) >= v.pack.World.MaxInventory }

// Add puts items into existing stacks, then new slots. Returns how many did
// not fit.
func (v inventory) Add(item string, qty int) int {
	def := v.pack.Item(item)
	if def == nil {
		return qty
	}
	for i := range v.c.Inventory {
		s := &v.c.Inventory[i]
		if s.Item == item && s.G == nil && s.Qty < def.Stack {
			n := min(qty, def.Stack-s.Qty)
			s.Qty += n
			qty -= n
		}
	}
	for qty > 0 && len(v.c.Inventory) < v.pack.World.MaxInventory {
		n := min(qty, def.Stack)
		v.c.Inventory = append(v.c.Inventory, content.Stack{Item: item, Qty: n})
		qty -= n
	}
	return qty
}

// AddGear stores a unique generated item. False if there is no room.
func (v inventory) AddGear(g *content.Gear) bool {
	if v.Full() {
		return false
	}
	v.c.Inventory = append(v.c.Inventory, content.Stack{Item: "gear:" + g.Base, Qty: 1, G: g})
	return true
}

// AddStack adds a stack of any kind (template or gear).
func (v inventory) AddStack(s content.Stack) bool {
	if s.G != nil {
		return v.AddGear(s.G)
	}
	return v.Add(s.Item, s.Qty) == 0
}

// Remove takes qty of an item from anywhere in the inventory.
func (v inventory) Remove(item string, qty int) bool {
	if v.Count(item) < qty {
		return false
	}
	for i := len(v.c.Inventory) - 1; i >= 0 && qty > 0; i-- {
		s := &v.c.Inventory[i]
		if s.Item != item || s.G != nil {
			continue
		}
		n := min(qty, s.Qty)
		s.Qty -= n
		qty -= n
	}
	v.compact()
	return true
}

// TakeAt removes up to qty from one slot and returns what was taken.
func (v inventory) TakeAt(index, qty int) (content.Stack, bool) {
	if index < 0 || index >= len(v.c.Inventory) {
		return content.Stack{}, false
	}
	s := &v.c.Inventory[index]
	n := min(qty, s.Qty)
	s.Qty -= n
	out := content.Stack{Item: s.Item, Qty: n, G: s.G}
	v.compact()
	return out, n > 0
}

func (v inventory) compact() {
	out := v.c.Inventory[:0]
	for _, s := range v.c.Inventory {
		if s.Qty > 0 {
			out = append(out, s)
		}
	}
	v.c.Inventory = out
}

// ToolTier is the best tier of the given tool kind the player carries.
func (v inventory) ToolTier(tool string) int {
	best := 0
	for _, s := range v.c.Inventory {
		if it := v.pack.Item(s.Item); it != nil && it.Type == "tool" && it.Tool == tool {
			best = max(best, it.Tier)
		}
	}
	return best
}
