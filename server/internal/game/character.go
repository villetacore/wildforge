package game

import (
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
)

// Character is the persistent part of a player, saved as JSON by storage.
type Character struct {
	Level     int             `json:"level"`
	XP        int             `json:"xp"`
	HP        float64         `json:"hp"`
	Gold      int             `json:"gold"`
	X         float64         `json:"x"`
	Y         float64         `json:"y"`
	Inventory []content.Stack `json:"inventory"`
	Weapon    string          `json:"weapon"`
	Armor     string          `json:"armor"`
	Kills     int             `json:"kills"`
}

func newCharacter(pack *content.Pack) *Character {
	c := &Character{Level: 1, HP: 100}
	for _, s := range pack.World.StartInventory {
		c.Inventory = append(c.Inventory, s)
	}
	return c
}

func (c *Character) clone() *Character {
	cp := *c
	cp.Inventory = append([]content.Stack(nil), c.Inventory...)
	return &cp
}

func (c *Character) MaxHP() float64 { return 100 + 20*float64(c.Level-1) }

func (c *Character) Attack(pack *content.Pack) int {
	a := 4 + 3*(c.Level-1)
	if it := pack.Item(c.Weapon); it != nil {
		a += it.Attack
	}
	return a
}

func (c *Character) Defense(pack *content.Pack) int {
	d := (c.Level - 1) / 2
	if it := pack.Item(c.Armor); it != nil {
		d += it.Defense
	}
	return d
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
		if s.Item == item {
			n += s.Qty
		}
	}
	return n
}

// Add puts items into existing stacks, then new slots. Returns how many did
// not fit.
func (v inventory) Add(item string, qty int) int {
	def := v.pack.Item(item)
	if def == nil {
		return qty
	}
	for i := range v.c.Inventory {
		s := &v.c.Inventory[i]
		if s.Item == item && s.Qty < def.Stack {
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

// Remove takes qty of an item from anywhere in the inventory.
func (v inventory) Remove(item string, qty int) bool {
	if v.Count(item) < qty {
		return false
	}
	for i := len(v.c.Inventory) - 1; i >= 0 && qty > 0; i-- {
		s := &v.c.Inventory[i]
		if s.Item != item {
			continue
		}
		n := min(qty, s.Qty)
		s.Qty -= n
		qty -= n
	}
	v.compact()
	return true
}

// TakeAt removes up to qty from one slot and returns the item id.
func (v inventory) TakeAt(index, qty int) (string, int) {
	if index < 0 || index >= len(v.c.Inventory) {
		return "", 0
	}
	s := &v.c.Inventory[index]
	n := min(qty, s.Qty)
	s.Qty -= n
	item := s.Item
	v.compact()
	return item, n
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
