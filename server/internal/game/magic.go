package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

// castSpell fires the spell bound to a spell bar slot.
func (g *Game) castSpell(p *Player, slot int) {
	if slot < 0 || slot >= len(p.Ch.SpellBar) {
		return
	}
	sp := g.pack.Spell(p.Ch.SpellBar[slot])
	if sp == nil || !p.Ch.knows(sp.ID) {
		return
	}
	c := p.Ch
	if g.mods(c.Statuses).stun {
		return
	}
	if p.SpellCD[sp.ID] > 0 {
		return
	}
	if c.Mana < sp.Mana {
		p.system("Not enough mana for " + sp.Name + ".")
		return
	}
	power := (sp.Power + p.S.SP*sp.Scale) * randRange(0.9, 1.1) * g.mods(c.Statuses).dealt
	crit := rand.Float64()*100 < p.S.Crit*0.6
	if crit {
		power *= 1.75
	}
	applyStatus := func(e *Enemy) {
		if sp.Status == "" {
			return
		}
		if rand.Float64() < sp.Chance || rand.Float64() < p.S.SpellStatus {
			def := g.pack.Status(sp.Status)
			mag := 1 + p.S.SP*0.08
			if def != nil && def.Dot > 0 && sp.Kind == "dot" {
				mag = power / def.Dot
			}
			g.addStatus(&e.Statuses, sp.Status, mag)
		}
	}
	color := sp.Color
	switch sp.Kind {
	case "bolt", "dot", "drain":
		id, e := g.pickTarget(p, sp.Range)
		if e == nil {
			p.system("No target in range.")
			return
		}
		p.Target = id
		g.fx = append(g.fx, fxBolt(c.X, c.Y, e.X, e.Y, color))
		if sp.Kind == "dot" {
			e.Target, e.LastHit = p.ID, p.ID
			applyStatus(e)
			g.addFx("status", e.X, e.Y, sp.Name)
		} else {
			applyStatus(e)
			g.damageEnemy(p, id, e, power, sp.School, crit, false)
			if sp.Kind == "drain" {
				g.healPlayer(p, power*0.6, true)
			}
		}
	case "aoe_target", "aoe_self":
		cx, cy := c.X, c.Y
		if sp.Kind == "aoe_target" {
			_, e := g.pickTarget(p, sp.Range)
			if e == nil {
				p.system("No target in range.")
				return
			}
			cx, cy = e.X, e.Y
			g.fx = append(g.fx, fxBolt(c.X, c.Y, cx, cy, color))
		}
		g.fx = append(g.fx, protocol.Fx{Kind: "nova", X: cx, Y: cy, R: sp.Radius, Color: color})
		for id, e := range g.enemies {
			if dist(e.X, e.Y, cx, cy) <= sp.Radius+0.3*e.Scale && g.world.ClearLine(cx, cy, e.X, e.Y) {
				applyStatus(e)
				g.damageEnemy(p, id, e, power, sp.School, crit, false)
			}
		}
	case "chain":
		id, e := g.pickTarget(p, sp.Range)
		if e == nil {
			p.system("No target in range.")
			return
		}
		hit := map[int64]bool{}
		fx, fy := c.X, c.Y
		for jump := 0; jump < max(1, sp.Chain) && e != nil; jump++ {
			hit[id] = true
			g.fx = append(g.fx, fxBolt(fx, fy, e.X, e.Y, color))
			fx, fy = e.X, e.Y
			applyStatus(e)
			g.damageEnemy(p, id, e, power*math.Pow(0.8, float64(jump)), sp.School, crit, false)
			var next int64
			best := 5.0
			for oid, o := range g.enemies {
				if !hit[oid] {
					if d := dist(o.X, o.Y, fx, fy); d < best {
						best, next = d, oid
					}
				}
			}
			id, e = next, g.enemies[next]
		}
	case "heal":
		g.healPlayer(p, power, true)
		g.fx = append(g.fx, protocol.Fx{Kind: "nova", X: c.X, Y: c.Y, R: 1.2, Color: color})
	case "buff":
		mag := 1.0
		if def := g.pack.Status(sp.Status); def != nil && def.Absorb > 0 {
			mag = def.Absorb + p.S.SP*1.5
		}
		g.addStatus(&c.Statuses, sp.Status, mag)
		g.fx = append(g.fx, protocol.Fx{Kind: "nova", X: c.X, Y: c.Y, R: 1.0, Color: color})
		g.addFx("status", c.X, c.Y, sp.Name)
	default:
		return
	}
	c.Mana -= sp.Mana
	p.SpellCD[sp.ID] = sp.CD * (1 - p.S.CDR)
	g.addFx("cast", c.X, c.Y, "")
}

// learnSpell teaches a spell from a tome.
func (g *Game) learnSpell(p *Player, id string) bool {
	sp := g.pack.Spell(id)
	if sp == nil {
		return false
	}
	if p.Ch.knows(id) {
		p.system("You already know " + sp.Name + ".")
		return false
	}
	if p.Ch.Level < sp.Level {
		p.system(fmt.Sprintf("%s requires level %d.", sp.Name, sp.Level))
		return false
	}
	p.Ch.Spells = append(p.Ch.Spells, id)
	for i, s := range p.Ch.SpellBar {
		if s == "" {
			p.Ch.SpellBar[i] = id
			break
		}
	}
	p.system("You learned " + sp.Name + "! Open the spellbook (B) to bind it.")
	g.addFx("level", p.Ch.X, p.Ch.Y, "New spell: "+sp.Name)
	return true
}

func (g *Game) spellBar(p *Player) []protocol.SpellSlot {
	out := make([]protocol.SpellSlot, len(p.Ch.SpellBar))
	for i, id := range p.Ch.SpellBar {
		if sp := g.pack.Spell(id); sp != nil {
			out[i] = protocol.SpellSlot{ID: id, Name: sp.Name, Color: sp.Color, CD: math.Max(0, p.SpellCD[id]), MaxCD: sp.CD * (1 - p.S.CDR), Mana: sp.Mana}
		}
	}
	return out
}

var _ = content.Spell{}
