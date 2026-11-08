package game

import (
	"fmt"
	"math"
)

const groundItemTTL = 90.0

func (g *Game) dropOnGround(item string, qty int, x, y float64) {
	if qty <= 0 {
		return
	}
	g.items[g.newID()] = &GroundItem{
		Item: item, Qty: qty, TTL: groundItemTTL,
		X: x + randRange(-0.5, 0.5), Y: y + randRange(-0.5, 0.5),
	}
}

func (g *Game) pickupNear(p *Player) {
	c := p.Ch
	for id, it := range g.items {
		if dist(c.X, c.Y, it.X, it.Y) >= pickupRange {
			continue
		}
		if it.Item == "gold" {
			c.Gold += it.Qty
			g.addFx("gold", it.X, it.Y, fmt.Sprintf("+%dg", it.Qty))
			delete(g.items, id)
			continue
		}
		left := c.inv(g.pack).Add(it.Item, it.Qty)
		if left == it.Qty {
			continue // inventory full
		}
		name := it.Item
		if def := g.pack.Item(it.Item); def != nil {
			name = def.Name
		}
		g.addFx("loot", it.X, it.Y, fmt.Sprintf("+%d %s", it.Qty-left, name))
		if left > 0 {
			it.Qty = left
		} else {
			delete(g.items, id)
		}
	}
}

func (g *Game) tickGroundItems() {
	for id, it := range g.items {
		if it.TTL -= dt; it.TTL <= 0 {
			delete(g.items, id)
		}
	}
}

// useItem drinks a potion or equips a weapon/armor from an inventory slot.
func (g *Game) useItem(p *Player, index int) {
	c := p.Ch
	if index < 0 || index >= len(c.Inventory) {
		return
	}
	id := c.Inventory[index].Item
	def := g.pack.Item(id)
	if def == nil {
		return
	}
	inv := c.inv(g.pack)
	switch def.Type {
	case "potion":
		mx := c.MaxHP()
		if c.HP >= mx {
			p.system("You are already at full health.")
			return
		}
		heal := math.Round(mx * def.Heal)
		c.HP = math.Min(c.HP+heal, mx)
		inv.TakeAt(index, 1)
		g.addFx("heal", c.X, c.Y, fmt.Sprintf("+%d", int(heal)))
	case "weapon", "armor":
		slot := &c.Weapon
		if def.Type == "armor" {
			slot = &c.Armor
		}
		inv.TakeAt(index, 1)
		if *slot != "" {
			inv.Add(*slot, 1)
		}
		*slot = id
		p.system("Equipped " + def.Name + ".")
	case "tool":
		p.system(def.Name + " works automatically while it is in your inventory.")
	default:
		p.system(def.Name + " is a crafting material. Press C to craft.")
	}
}

func (g *Game) quickPotion(p *Player) {
	for i, s := range p.Ch.Inventory {
		if def := g.pack.Item(s.Item); def != nil && def.Type == "potion" {
			g.useItem(p, i)
			return
		}
	}
	p.system("You have no potions.")
}

func (g *Game) dropFromInventory(p *Player, index int) {
	if index < 0 || index >= len(p.Ch.Inventory) {
		return
	}
	qty := p.Ch.Inventory[index].Qty
	item, n := p.Ch.inv(g.pack).TakeAt(index, qty)
	// Put it a little away so it is not picked up again immediately.
	g.items[g.newID()] = &GroundItem{Item: item, Qty: n, X: p.Ch.X + 1.2, Y: p.Ch.Y, TTL: 120}
}

func (g *Game) unequip(p *Player, slot string) {
	c := p.Ch
	var s *string
	switch slot {
	case "weapon":
		s = &c.Weapon
	case "armor":
		s = &c.Armor
	default:
		return
	}
	if *s == "" {
		return
	}
	if c.inv(g.pack).Add(*s, 1) > 0 {
		p.system("Inventory full.")
		return
	}
	*s = ""
}
