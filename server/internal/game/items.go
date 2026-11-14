package game

import (
	"fmt"
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
)

const groundItemTTL = 120.0

func (g *Game) dropOnGround(s content.Stack, x, y float64) {
	if s.Qty <= 0 {
		return
	}
	g.items[g.newID()] = &GroundItem{
		Stack: s, TTL: groundItemTTL,
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
		if it.G != nil {
			if !c.inv(g.pack).AddGear(it.G) {
				continue
			}
			g.addFx("loot", it.X, it.Y, "+"+it.G.Name)
			delete(g.items, id)
			continue
		}
		left := c.inv(g.pack).Add(it.Item, it.Qty)
		if left == it.Qty {
			continue // inventory full
		}
		g.addFx("loot", it.X, it.Y, fmt.Sprintf("+%d %s", it.Qty-left, g.stackName(&it.Stack)))
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

// useItem consumes, learns, adopts or equips an inventory slot.
func (g *Game) useItem(p *Player, index int) {
	c := p.Ch
	if index < 0 || index >= len(c.Inventory) {
		return
	}
	st := c.Inventory[index]
	inv := c.inv(g.pack)
	if slot := g.equipSlot(&st); slot != "" {
		taken, _ := inv.TakeAt(index, 1)
		if old := c.Equip[slot]; old != nil {
			inv.AddStack(*old)
		}
		c.Equip[slot] = &taken
		p.S = g.computeStats(c)
		p.system("Equipped " + g.stackName(&taken) + ".")
		return
	}
	def := g.pack.Item(st.Item)
	if def == nil {
		return
	}
	switch def.Type {
	case "potion", "food":
		used := false
		if def.Heal > 0 && c.HP < p.S.MaxHP {
			heal := math.Round(p.S.MaxHP * def.Heal * (1 + p.S.Potion))
			g.healPlayer(p, heal, true)
			used = true
		}
		if def.Mana > 0 && c.Mana < p.S.MaxMana {
			c.Mana = math.Min(p.S.MaxMana, c.Mana+p.S.MaxMana*def.Mana*(1+p.S.Potion))
			g.addFx("xp", c.X, c.Y, "+mana")
			used = true
		}
		if def.Cleanse {
			if removeStatuses(&c.Statuses, func(s Status) bool { d := g.pack.Status(s.ID); return d == nil || !d.Bad || d.Disease }) > 0 {
				used = true
			}
		}
		if def.Cure {
			if removeStatuses(&c.Statuses, func(s Status) bool { d := g.pack.Status(s.ID); return d == nil || !d.Disease }) > 0 {
				p.system("The disease is gone. What a relief.")
				used = true
			}
		}
		if def.Mood > 0 {
			c.Mood = math.Min(100, c.Mood+def.Mood)
			used = true
		}
		if def.Status != "" {
			g.addStatus(&c.Statuses, def.Status, 1)
			used = true
		}
		if def.PetHeal > 0 {
			if p.Pet != nil {
				p.Pet.HP = p.Pet.MaxHP
				g.addFx("heal", p.Pet.X, p.Pet.Y, "♥")
				used = true
			} else {
				p.system("Summon a pet first (H).")
			}
		}
		if !used {
			p.system("That would have no effect right now.")
			return
		}
		inv.TakeAt(index, 1)
	case "tome":
		if g.learnSpell(p, def.Spell) {
			inv.TakeAt(index, 1)
		}
	case "pet":
		if pd := g.pack.PetDef(def.Pet); pd != nil && g.adoptPet(p, newPetFromDef(pd)) {
			inv.TakeAt(index, 1)
		}
	case "tool":
		p.system(def.Name + " works automatically while it is in your inventory.")
	default:
		p.system(def.Name + " is a crafting material. Press C to craft or visit a smith.")
	}
}

func (g *Game) quickPotion(p *Player) {
	for i, s := range p.Ch.Inventory {
		if def := g.pack.Item(s.Item); def != nil && s.G == nil && def.Type == "potion" && def.Heal > 0 {
			g.useItem(p, i)
			return
		}
	}
	p.system("You have no healing potions.")
}

func (g *Game) dropFromInventory(p *Player, index int) {
	if index < 0 || index >= len(p.Ch.Inventory) {
		return
	}
	qty := p.Ch.Inventory[index].Qty
	s, ok := p.Ch.inv(g.pack).TakeAt(index, qty)
	if !ok {
		return
	}
	// Put it a little away so it is not picked up again immediately.
	g.items[g.newID()] = &GroundItem{Stack: s, X: p.Ch.X + 1.2, Y: p.Ch.Y, TTL: 180}
}

func (g *Game) unequip(p *Player, slot string) {
	c := p.Ch
	s := c.Equip[slot]
	if s == nil {
		return
	}
	if !c.inv(g.pack).AddStack(*s) {
		p.system("Inventory full.")
		return
	}
	delete(c.Equip, slot)
	p.S = g.computeStats(c)
}
