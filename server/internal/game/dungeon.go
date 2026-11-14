package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/world"
)

const (
	dungeonMaxAge   = 1500.0 // seconds before an untouched dungeon reshuffles
	dungeonBossWait = 120.0  // seconds after the boss dies
	chestRefill     = 300.0
)

type dungeonState struct {
	age      float64
	bossDead float64 // seconds since the boss died; < 0 while alive
}

func (g *Game) interactDungeon(p *Player) bool {
	if p.GatherCD > 0 {
		return false
	}
	for i, link := range g.world.Portals {
		if dist(p.Ch.X, p.Ch.Y, link.X, link.Y) > 1.4 {
			continue
		}
		p.Ch.X, p.Ch.Y = link.ToX, link.ToY
		p.DirX, p.DirY = 0, 0
		p.GatherCD = 2
		g.forgetTarget(p.ID)
		if p.Pet != nil {
			p.Pet.X, p.Pet.Y = p.Ch.X, p.Ch.Y
		}
		if d, ok := g.world.DungeonAt(link.ToX, link.ToY); ok {
			z := g.pack.Zones[d.Zone]
			p.system(fmt.Sprintf("You descend into %s (level %d+). The halls shift every time they reset. E opens caches; E at the glowing arch returns to the surface.", d.Name, z.Dungeon.Level))
		} else {
			p.system("Returned to the surface.")
		}
		_ = i
		return true
	}
	x, y := int(math.Floor(p.Ch.X)), int(math.Floor(p.Ch.Y))
	for yy := y - 1; yy <= y+1; yy++ {
		for xx := x - 1; xx <= x+1; xx++ {
			if !g.world.InBounds(xx, yy) {
				continue
			}
			i := g.world.Index(xx, yy)
			cx, cy := float64(xx)+.5, float64(yy)+.5
			if g.world.Terrain[i] != world.Chest || dist(p.Ch.X, p.Ch.Y, cx, cy) > 1.5 || !g.world.ClearLine(p.Ch.X, p.Ch.Y, cx, cy) {
				continue
			}
			p.GatherCD = 1
			if g.chestTimers[i] > 0 {
				p.system("This cache is empty. It replenishes in a few minutes.")
				return true
			}
			// Inventory-full leaves the cache available and does not grant partial rewards.
			if p.Ch.inv(g.pack).Add("potion", 1) > 0 {
				p.system("Inventory full.")
				return true
			}
			level := p.Ch.Level
			boss := false
			if d, ok := g.world.DungeonAt(cx, cy); ok {
				level = max(level, g.pack.Zones[d.Zone].Dungeon.Level)
				if len(d.Rooms) > d.Boss {
					r := d.Rooms[d.Boss]
					boss = xx >= r.X && yy >= r.Y && xx < r.X+r.W && yy < r.Y+r.H
				}
			}
			gold := 15 + randInt(0, 20) + level*3
			if boss {
				gold *= 4
			}
			p.Ch.Gold += gold
			g.chestTimers[i] = chestRefill
			msg := fmt.Sprintf("Vault cache: +%d gold and a healing potion.", gold)
			chance := 0.35 + p.S.MF*0.3
			if boss {
				chance = 1.5
			}
			for chance > 0 {
				if rand.Float64() < chance {
					minR := 0
					if boss {
						minR = 2
					}
					gear := g.genGear(level, GearSpec{Quality: -1, Rarity: -1, MinRarity: minR, RarityBoost: 0.8 + p.S.MF})
					if p.Ch.inv(g.pack).AddGear(gear) {
						msg += fmt.Sprintf(" Found %s!", gear.Name)
					} else {
						g.dropOnGround(content.Stack{Item: "gear:" + gear.Base, Qty: 1, G: gear}, p.Ch.X, p.Ch.Y)
					}
				}
				chance--
			}
			p.system(msg)
			g.addFx("gold", cx, cy, fmt.Sprintf("+%dg", gold))
			return true
		}
	}
	return false
}

// spawnDungeon populates a freshly generated dungeon.
func (g *Game) spawnDungeon(i int) {
	d := g.world.Dungeons[i]
	z := g.pack.Zones[d.Zone]
	fams := z.Dungeon.Families
	for _, s := range d.Spawns {
		x, y := float64(s[0])+.5, float64(s[1])+.5
		level := z.Dungeon.Level + rand.IntN(3)
		var e *Enemy
		if s[2] == 1 {
			fam := g.pack.Family(fams[rand.IntN(len(fams))])
			rank := g.rankIndex("boss")
			e = g.makeEnemy(fam, g.rollElement(fam, z.Elements), rank, level+2, g.rollTraits(rank), x, y)
			e.Name = g.pack.Bestiary.BossNames[rand.IntN(len(g.pack.Bestiary.BossNames))] + ", " + e.Name
			e.Boss = true
		} else {
			e = g.randomEnemy(d.Zone, fams, level, x, y)
			if e.Rank == g.rankIndex("feeble") {
				e = g.makeEnemy(e.Fam, e.Elem, 1, level, e.Traits, x, y)
			}
		}
		e.Dungeon = i
		e.Zone = d.Zone
		g.enemies[g.newID()] = e
	}
	g.dungeons[i] = &dungeonState{bossDead: -1}
}

func (g *Game) onBossKilled(p *Player, e *Enemy) {
	st := g.dungeons[e.Dungeon]
	if st != nil {
		st.bossDead = 0
	}
	p.Ch.Bosses++
	d := g.world.Dungeons[e.Dungeon]
	g.broadcastSystem(fmt.Sprintf("%s has slain %s in %s!", p.Name, e.Name, d.Name))
	g.changeRep(p, g.world.Towns[d.Zone].Faction, 400)
	if d.Zone == 0 {
		g.changeRep(p, "crown", 200)
	}
}

// tickDungeons reshuffles empty dungeons whose boss died or that aged out.
func (g *Game) tickDungeons() {
	for i, st := range g.dungeons {
		st.age += dt
		if st.bossDead >= 0 {
			st.bossDead += dt
		}
		if !(st.bossDead > dungeonBossWait || st.age > dungeonMaxAge) {
			continue
		}
		d := g.world.Dungeons[i]
		occupied := false
		for _, p := range g.players {
			if d.Contains(p.Ch.X, p.Ch.Y) {
				occupied = true
			}
		}
		if occupied {
			continue
		}
		g.resetDungeon(i)
	}
}

func (g *Game) resetDungeon(i int) {
	d := g.world.Dungeons[i]
	for id, e := range g.enemies {
		if e.Dungeon == i {
			delete(g.enemies, id)
		}
	}
	for idx := range g.chestTimers {
		if d.Contains(float64(idx%g.world.Size), float64(idx/g.world.Size)) {
			delete(g.chestTimers, idx)
		}
	}
	g.world.RegenDungeon(i, rand.Uint32())
	g.spawnDungeon(i)
	g.broadcast(protocol.DungeonPatch{T: "dungeon", Index: i, X: d.X0, Y: d.Y0, Size: d.Size, Terrain: g.world.EncodeRect(d.X0, d.Y0, d.Size), Portals: g.world.Portals})
}
