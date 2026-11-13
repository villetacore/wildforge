package game

import (
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/content"
)

type Enemy struct {
	Dungeon      bool
	Def          *content.Enemy
	Level        int
	HP, MaxHP    float64
	X, Y         float64
	HomeX, HomeY float64
	Target       int64
	AttackCD     float64
	WX, WY       float64
	WanderT      float64
}

// spawnEnemy places an enemy on a random free tile away from town and players.
// Enemies get stronger the further from the island centre they spawn.
func (g *Game) spawnEnemy() {
	w := g.world
	size := float64(w.Size)
	safe := g.pack.World.SpawnSafeRadius
outer:
	for range 200 {
		x, y := randRange(2, size-2), randRange(2, size-2)
		d := dist(x, y, w.SpawnX, w.SpawnY)
		if d < safe || w.Underground(x, y) || !w.Free(x, y) {
			continue
		}
		for _, p := range g.players {
			if dist(p.Ch.X, p.Ch.Y, x, y) < 12 {
				continue outer
			}
		}
		level := 1 + int(d/(size/28)) + rand.IntN(2)
		var cands []*content.Enemy
		for i := range g.pack.Enemies {
			if g.pack.Enemies[i].MinLevel <= level {
				cands = append(cands, &g.pack.Enemies[i])
			}
		}
		if len(cands) == 0 {
			cands = append(cands, &g.pack.Enemies[0])
		}
		def := cands[rand.IntN(len(cands))]
		hp := def.HP + def.HPPerLevel*float64(level-1)
		g.enemies[g.newID()] = &Enemy{Def: def, Level: level, HP: hp, MaxHP: hp, X: x, Y: y, HomeX: x, HomeY: y}
		return
	}
}

func (g *Game) forgetTarget(playerID int64) {
	for _, e := range g.enemies {
		if e.Target == playerID {
			e.Target = 0
		}
	}
}

// tickEnemies runs a simple chase / wander AI.
func (g *Game) tickEnemies() {
	for _, e := range g.enemies {
		e.AttackCD -= dt
		if e.Target == 0 {
			best := aggroRange
			for id, p := range g.players {
				if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y); d < best && g.world.ClearLine(p.Ch.X, p.Ch.Y, e.X, e.Y) {
					best, e.Target = d, id
				}
			}
		}
		t, chasing := g.players[e.Target]
		if chasing && dist(e.X, e.Y, e.HomeX, e.HomeY) < leashRange {
			d := dist(e.X, e.Y, t.Ch.X, t.Ch.Y)
			reach := 0.9 * math.Max(1, e.Def.Scale)
			if d > reach {
				s := e.Def.Speed * dt / d
				e.X, e.Y = g.world.Step(e.X, e.Y, (t.Ch.X-e.X)*s, (t.Ch.Y-e.Y)*s)
			} else if e.AttackCD <= 0 && g.world.ClearLine(e.X, e.Y, t.Ch.X, t.Ch.Y) {
				e.AttackCD = 1
				g.enemyHit(t, e.Def.Damage+e.Def.DamagePerLevel*float64(e.Level-1))
			}
			if d > aggroRange*2 {
				e.Target = 0
			}
			continue
		}
		// Wander, drift home if too far, heal while idle.
		e.Target = 0
		e.HP = math.Min(e.HP+e.MaxHP*0.05*dt, e.MaxHP)
		e.WanderT -= dt
		if hd := dist(e.X, e.Y, e.HomeX, e.HomeY); hd > 6 {
			e.WX, e.WY = (e.HomeX-e.X)/hd, (e.HomeY-e.Y)/hd
		} else if e.WanderT <= 0 {
			e.WanderT = randRange(1, 4)
			if rand.Float64() < 0.4 {
				e.WX, e.WY = 0, 0
			} else {
				a := rand.Float64() * 2 * math.Pi
				e.WX, e.WY = math.Cos(a), math.Sin(a)
			}
		}
		s := e.Def.Speed * 0.4 * dt
		e.X, e.Y = g.world.Step(e.X, e.Y, e.WX*s, e.WY*s)
	}
}
