package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"strconv"
	"strings"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

const maxContracts = 5

// refreshContracts regenerates each town board's offers every ten minutes.
func (g *Game) refreshContracts() {
	g.boards = make([][]Contract, len(g.world.Towns))
	for ti := range g.world.Towns {
		for k := 0; k < 4; k++ {
			g.boards[ti] = append(g.boards[ti], g.genContract(ti, k%2 == 0))
		}
	}
}

func (g *Game) genContract(ti int, hunt bool) Contract {
	t := g.world.Towns[ti]
	z := g.pack.Zones[t.Zone]
	fam := g.pack.Family(z.Families[rand.IntN(len(z.Families))])
	level := z.MinLevel + rand.IntN(max(1, z.MaxLevel-z.MinLevel+1))
	c := Contract{ID: g.newID(), Town: ti, Zone: t.Zone, Family: fam.ID, Level: level, Faction: t.Faction}
	if hunt {
		boss := g.pack.Bestiary.BossNames[rand.IntN(len(g.pack.Bestiary.BossNames))]
		c.Kind = "hunt"
		c.Target = fmt.Sprintf("%s the %s", boss, fam.Name)
		c.Title = "Wanted: " + c.Target
		// Somewhere in the zone, away from the town.
		for range 100 {
			a := rand.Float64() * 2 * math.Pi
			r := t.R + 25 + rand.Float64()*70
			x, y := t.X+math.Cos(a)*r, t.Y+math.Sin(a)*r
			if g.world.Free(x, y) && !g.world.Underground(x, y) && g.world.ZoneOf(x, y) == t.Zone {
				c.X, c.Y = x, y
				break
			}
		}
		if c.X == 0 {
			c.X, c.Y = t.X+t.R+20, t.Y
		}
		c.Desc = fmt.Sprintf("A monstrous %s has been terrorising travellers near %s. Slay it and bring proof to any contract board.", strings.ToLower(fam.Name), t.Name)
		c.Need, c.Gold, c.XP, c.Rep = 1, level*30+60, level*80, 300
	} else {
		c.Kind = "cull"
		c.Need = 6 + rand.IntN(8)
		c.Title = fmt.Sprintf("Cull: %d %ss in %s", c.Need, fam.Name, z.Name)
		c.Desc = fmt.Sprintf("The %s want the %s population thinned. Any %s in %s counts.", g.factionName(t.Faction), strings.ToLower(fam.Name), strings.ToLower(fam.Name), z.Name)
		c.Gold, c.XP, c.Rep = level*12+c.Need*4, level*25*c.Need/4, 150
	}
	return c
}

// spawnContractTarget creates the named monster for a hunt contract.
func (g *Game) spawnContractTarget(p *Player, c *Contract) {
	for _, e := range g.enemies {
		if e.ContractID == c.ID {
			return
		}
	}
	fam := g.pack.Family(c.Family)
	if fam == nil {
		return
	}
	z := g.pack.Zones[c.Zone]
	rank := g.rankIndex("champion")
	if rand.Float64() < 0.5 {
		rank = g.rankIndex("elite")
	}
	e := g.makeEnemy(fam, g.rollElement(fam, z.Elements), rank, c.Level+1, g.rollTraits(rank), c.X, c.Y)
	e.Name = c.Target
	e.ContractOwner, e.ContractID, e.Zone = p.ID, c.ID, c.Zone
	e.Loot *= 1.5
	g.enemies[g.newID()] = e
}

func (g *Game) ensureContractTargets(p *Player) {
	for i := range p.Ch.Contracts {
		c := &p.Ch.Contracts[i]
		if c.Kind == "hunt" && !c.Done {
			g.spawnContractTarget(p, c)
		}
	}
}

func (g *Game) contractKill(p *Player, e *Enemy) {
	for i := range p.Ch.Contracts {
		c := &p.Ch.Contracts[i]
		if c.Done {
			continue
		}
		switch {
		case c.Kind == "hunt" && e.ContractID == c.ID:
			c.Have, c.Done = 1, true
		case c.Kind == "cull" && e.Fam.ID == c.Family && e.Zone == c.Zone && e.Dungeon < 0:
			c.Have++
			c.Done = c.Have >= c.Need
		default:
			continue
		}
		if c.Done {
			p.system(fmt.Sprintf("Contract complete: %s. Return to any contract board for your reward.", c.Title))
			g.addFx("level", p.Ch.X, p.Ch.Y, "Contract complete!")
		} else {
			g.addFx("loot", p.Ch.X, p.Ch.Y, fmt.Sprintf("%s %d/%d", c.Family, c.Have, c.Need))
		}
	}
}

func (g *Game) contractBoard(p *Player, n *NPC, opt string) {
	msg := "Parchments flutter on rusty nails. Bounties, hunts and desperate pleas for help."
	ti := n.Town
	if id, ok := strings.CutPrefix(opt, "accept:"); ok {
		cid, _ := strconv.ParseInt(id, 10, 64)
		for k, c := range g.boards[ti] {
			if c.ID != cid {
				continue
			}
			if len(p.Ch.Contracts) >= maxContracts {
				msg = "[color=#ff7766]You can't take more than 5 contracts at once.[/color]"
				break
			}
			p.Ch.Contracts = append(p.Ch.Contracts, c)
			g.boards[ti][k] = g.genContract(ti, c.Kind == "hunt")
			if c.Kind == "hunt" {
				g.spawnContractTarget(p, &p.Ch.Contracts[len(p.Ch.Contracts)-1])
			}
			msg = "You tear the notice from the board. Contract accepted: [color=#ffd27a]" + c.Title + "[/color]. Its target is marked on your map."
		}
	}
	if id, ok := strings.CutPrefix(opt, "turnin:"); ok {
		cid, _ := strconv.ParseInt(id, 10, 64)
		for k, c := range p.Ch.Contracts {
			if c.ID != cid || !c.Done {
				continue
			}
			gold := int(float64(c.Gold) * (1 + p.S.Contract))
			p.Ch.Gold += gold
			g.giveXP(p, c.XP)
			g.changeRep(p, c.Faction, c.Rep)
			p.Ch.Contracts = append(p.Ch.Contracts[:k], p.Ch.Contracts[k+1:]...)
			g.addFx("gold", p.Ch.X, p.Ch.Y, fmt.Sprintf("+%dg", gold))
			msg = fmt.Sprintf("Reward collected: [color=#ffd27a]%d gold[/color], %d XP and the gratitude of the %s.", gold, c.XP, g.factionName(c.Faction))
			break
		}
	}
	d := newDialog(n, "", "Contract Board", msg)
	d.Sub = g.world.Towns[ti].Name
	for _, c := range p.Ch.Contracts {
		if c.Done {
			d.optx(protocol.Option{ID: fmt.Sprintf("turnin:%d", c.ID), Label: "✔ Collect reward: " + c.Title, Color: "#8aff8a"})
		}
	}
	for _, c := range g.boards[ti] {
		hint := fmt.Sprintf("%s\nLevel %d · %dg · %d XP · +%d %s reputation", c.Desc, c.Level, int(float64(c.Gold)*(1+p.S.Contract)), c.XP, c.Rep, g.factionName(c.Faction))
		color := "#e0d0a0"
		if c.Kind == "hunt" {
			color = "#ff9a6a"
		}
		d.optx(protocol.Option{ID: fmt.Sprintf("accept:%d", c.ID), Label: fmt.Sprintf("[lvl %d] %s", c.Level, c.Title), Hint: hint, Color: color, Disabled: len(p.Ch.Contracts) >= maxContracts})
	}
	g.show(p, d.bye())
}

// contractMarks shows active hunt targets and completed contracts on the map.
func (g *Game) contractMarks(p *Player) []protocol.Mark {
	marks := []protocol.Mark{}
	for _, c := range p.Ch.Contracts {
		if c.Done {
			t := g.world.Towns[c.Town]
			marks = append(marks, protocol.Mark{X: t.X, Y: t.Y, Label: "Reward: " + c.Title, Color: "#8aff8a"})
			continue
		}
		if c.Kind == "hunt" {
			x, y := c.X, c.Y
			for _, e := range g.enemies {
				if e.ContractID == c.ID {
					x, y = e.X, e.Y
				}
			}
			marks = append(marks, protocol.Mark{X: x, Y: y, Label: c.Target, Color: "#ff6a4a"})
		}
	}
	for _, d := range g.world.Dungeons {
		marks = append(marks, protocol.Mark{X: d.SurfaceX, Y: d.SurfaceY, Label: d.Name, Color: "#7ad8ff"})
	}
	return marks
}
