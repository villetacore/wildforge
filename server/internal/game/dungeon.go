package game

import (
	"fmt"
	"github.com/villetacore/wildforge/server/internal/world"
	"math"
)

func (g *Game) interactDungeon(p *Player) bool {
	if p.GatherCD > 0 {
		return true
	}
	for _, link := range g.world.Portals {
		if dist(p.Ch.X, p.Ch.Y, link.X, link.Y) > 1.4 {
			continue
		}
		p.Ch.X, p.Ch.Y = link.ToX, link.ToY
		p.DirX, p.DirY = 0, 0
		p.GatherCD = 2
		g.forgetTarget(p.ID)
		if g.world.Underground(link.ToX, link.ToY) {
			p.system("Entered the forgotten vault. E opens caches; E at the blue arch returns to the surface.")
		} else {
			p.system("Returned to the surface.")
		}
		return true
	}
	x, y := int(math.Floor(p.Ch.X)), int(math.Floor(p.Ch.Y))
	for yy := y - 1; yy <= y+1; yy++ {
		for xx := x - 1; xx <= x+1; xx++ {
			if !g.world.InBounds(xx, yy) {
				continue
			}
			i := g.world.Index(xx, yy)
			if !g.world.ClearLine(p.Ch.X, p.Ch.Y, float64(xx)+.5, float64(yy)+.5) || g.world.Terrain[i] != world.Chest || dist(p.Ch.X, p.Ch.Y, float64(xx)+.5, float64(yy)+.5) > 1.5 {
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
			gold := 15 + randInt(0, 20)
			p.Ch.Gold += gold
			g.chestTimers[i] = 300
			p.system(fmt.Sprintf("Vault cache: +%d gold and a healing potion.", gold))
			g.addFx("gold", float64(xx)+.5, float64(yy)+.5, fmt.Sprintf("+%dg", gold))
			return true
		}
	}
	return false
}

// Seed guardians at caches so every vault has opponents regardless of the
// overworld population's random distribution.
func (g *Game) spawnDungeonGuards() {
	if len(g.pack.Enemies) == 0 {
		return
	}
	for idx, code := range g.world.Terrain {
		if code != world.Chest {
			continue
		}
		x, y := float64(idx%g.world.Size)+.5, float64(idx/g.world.Size)+.5
		safe := false
		for _, p := range g.world.Portals {
			if dist(x, y, p.X, p.Y) < 9 {
				safe = true
			}
		}
		if safe {
			continue
		}
		def := &g.pack.Enemies[0]
		for i := range g.pack.Enemies {
			if g.pack.Enemies[i].ID == "skeleton" {
				def = &g.pack.Enemies[i]
				break
			}
		}
		hp := def.HP
		g.enemies[g.newID()] = &Enemy{Dungeon: true, Def: def, Level: 1, HP: hp, MaxHP: hp, X: x, Y: y, HomeX: x, HomeY: y}
	}
}
