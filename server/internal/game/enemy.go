package game

import (
	"math"
	"math/rand/v2"
	"strings"

	"github.com/villetacore/wildforge/server/internal/content"
)

type hitEffect struct {
	status string
	chance float64
}

// Enemy is assembled from a family, an element, a rank and a set of traits.
type Enemy struct {
	Fam    *content.Family
	Elem   *content.Element
	Rank   int
	Traits []*content.Trait
	Name   string
	Level  int
	Zone   int

	HP, MaxHP, Dmg, Speed, Armor        float64
	XP, Scale, Regen, Lifesteal, Thorns float64
	Dodge, AtkSpeed, Explode            float64
	GoldMult, Loot                      float64
	OnHit                               []hitEffect
	Color, Glow                         string

	X, Y, HomeX, HomeY float64
	Target             int64 // player id
	PetTarget          int64 // pet id (taunted)
	TauntT             float64
	AttackCD           float64
	WX, WY, WanderT    float64
	Statuses           []Status
	LastHit            int64 // player credited with a kill by damage over time

	Dungeon       int // -1 on the surface
	Boss          bool
	ContractOwner int64
	ContractID    int64
	Respawns      bool // surface population that refills on death
}

func (g *Game) rankDef(i int) *content.Rank { return &g.pack.Bestiary.Ranks[i] }

func (g *Game) rankIndex(id string) int {
	for i, r := range g.pack.Bestiary.Ranks {
		if r.ID == id {
			return i
		}
	}
	return 1
}

// makeEnemy combines bestiary parts into a concrete monster.
func (g *Game) makeEnemy(fam *content.Family, elem *content.Element, rank, level int, traits []*content.Trait, x, y float64) *Enemy {
	r := g.rankDef(rank)
	e := &Enemy{
		Fam: fam, Elem: elem, Rank: rank, Traits: traits, Level: max(1, level),
		X: x, Y: y, HomeX: x, HomeY: y, Dungeon: -1, AtkSpeed: 1, GoldMult: 1, Loot: r.Loot,
		Color: fam.Color, Glow: elem.Color,
	}
	lv := float64(e.Level - 1)
	hpMul, dmgMul, spd, scale, xp := r.HP*elemOr(elem.HP), r.Dmg*elemOr(elem.Dmg), 1.0, fam.Scale*r.Scale, r.XP
	e.Armor = fam.Armor + elem.Armor + lv*0.25
	e.Lifesteal = elem.Lifesteal
	if elem.Status != "" {
		e.OnHit = append(e.OnHit, hitEffect{elem.Status, elem.Chance})
	}
	for _, t := range traits {
		hpMul *= traitOr(t.HP)
		dmgMul *= traitOr(t.Dmg)
		spd *= traitOr(t.Speed)
		scale *= traitOr(t.Scale)
		xp *= traitOr(t.XP)
		e.Armor += t.Armor
		e.Regen += t.Regen
		e.Lifesteal += t.Lifesteal
		e.Thorns += t.Thorns
		e.Dodge += t.Dodge
		e.Explode = math.Max(e.Explode, t.Explode)
		if t.AttackSpeed > 0 {
			e.AtkSpeed *= t.AttackSpeed
		}
		if t.Gold > 0 {
			e.GoldMult *= t.Gold
		}
		if t.Loot > 0 {
			e.Loot *= t.Loot
		}
		if t.Status != "" {
			e.OnHit = append(e.OnHit, hitEffect{t.Status, t.Chance})
		}
	}
	e.MaxHP = math.Round((fam.HP + fam.HPLvl*lv) * hpMul)
	e.HP = e.MaxHP
	e.Dmg = (fam.Dmg + fam.DmgLvl*lv) * dmgMul
	e.Speed = fam.Speed * spd
	e.Scale = math.Min(scale, 3.2)
	e.XP = float64(fam.XP) * xp
	e.Name = g.enemyName(e)
	return e
}

func elemOr(v float64) float64 {
	if v == 0 {
		return 1
	}
	return v
}

func traitOr(v float64) float64 {
	if v == 0 {
		return 1
	}
	return v
}

func (g *Game) enemyName(e *Enemy) string {
	parts := []string{}
	if n := g.rankDef(e.Rank).Name; n != "" {
		parts = append(parts, n)
	}
	for i, t := range e.Traits {
		if i < 2 {
			parts = append(parts, t.Name)
		}
	}
	if e.Elem.Name != "" {
		parts = append(parts, e.Elem.Name)
	}
	parts = append(parts, e.Fam.Name)
	return strings.Join(parts, " ")
}

func (g *Game) rollTraits(rank int) []*content.Trait {
	r := g.rankDef(rank)
	n := r.Traits[0]
	if r.Traits[1] > n {
		n += rand.IntN(r.Traits[1] - n + 1)
	}
	all := g.pack.Bestiary.Traits
	var out []*content.Trait
	for _, i := range rand.Perm(len(all))[:min(n, len(all))] {
		out = append(out, &all[i])
	}
	return out
}

func (g *Game) rollElement(fam *content.Family, weights map[string]float64) *content.Element {
	type cand struct {
		e *content.Element
		w float64
	}
	var cands []cand
	for _, id := range fam.Elements {
		w := weights[id]
		if w == 0 {
			w = 0.3
		}
		cands = append(cands, cand{g.pack.Element(id), w})
	}
	c := weighted(cands, func(c *cand) float64 { return c.w })
	return c.e
}

func (g *Game) rollRank() int {
	ranks := g.pack.Bestiary.Ranks
	r := weighted(ranks, func(r *content.Rank) float64 { return r.Weight })
	return g.rankIndex(r.ID)
}

// randomEnemy rolls a monster for a zone at a given level.
func (g *Game) randomEnemy(zone int, families []string, level int, x, y float64) *Enemy {
	z := g.pack.Zones[zone]
	fam := g.pack.Family(families[rand.IntN(len(families))])
	elem := g.rollElement(fam, z.Elements)
	rank := g.rollRank()
	return g.makeEnemy(fam, elem, rank, level, g.rollTraits(rank), x, y)
}

// spawnEnemy places a monster on a random free tile away from towns and
// players. Level follows the zone's range, rising with distance from its town.
func (g *Game) spawnEnemy() {
	w := g.world
	size := float64(w.Size)
outer:
	for range 200 {
		x, y := randRange(2, size-2), randRange(2, size-2)
		if w.Underground(x, y) || !w.Free(x, y) {
			continue
		}
		if _, inTown := w.TownAt(x, y); inTown {
			continue
		}
		for _, t := range w.Towns {
			if dist(x, y, t.X, t.Y) < t.R+8 {
				continue outer
			}
		}
		for _, p := range g.players {
			if dist(p.Ch.X, p.Ch.Y, x, y) < 14 {
				continue outer
			}
		}
		zi := w.ZoneOf(x, y)
		z := g.pack.Zones[zi]
		t := w.Towns[zi]
		f := math.Min(1, math.Max(0, (dist(x, y, t.X, t.Y)-t.R)/110))
		level := z.MinLevel + int(f*float64(z.MaxLevel-z.MinLevel)+rand.Float64()*1.5)
		e := g.randomEnemy(zi, z.Families, level, x, y)
		e.Zone = zi
		e.Respawns = true
		g.enemies[g.newID()] = e
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

// enemyStep moves an enemy but never into a town's safe area.
func (g *Game) enemyStep(e *Enemy, dx, dy float64) {
	nx, ny := g.world.Step(e.X, e.Y, dx, dy)
	if _, inTown := g.world.TownAt(nx, ny); inTown && e.Dungeon < 0 {
		return
	}
	e.X, e.Y = nx, ny
}

// tickEnemies runs statuses and a chase / ranged / wander AI.
func (g *Game) tickEnemies() {
	for id, e := range g.enemies {
		dot, _ := g.tickStatuses(&e.Statuses)
		if dot > 0 {
			e.HP -= dot
			if e.HP <= 0 {
				if p, ok := g.players[e.LastHit]; ok {
					g.killEnemy(p, id, e)
				} else {
					e.HP = 1
				}
				continue
			}
		}
		m := g.mods(e.Statuses)
		e.AttackCD -= dt
		if e.Regen > 0 {
			e.HP = math.Min(e.MaxHP, e.HP+e.MaxHP*e.Regen*dt)
		}
		if m.stun {
			continue
		}
		speed := e.Speed * (1 - m.slow)
		leash := leashRange
		if e.Dungeon >= 0 {
			leash = 12
		}
		if e.ContractOwner != 0 || e.Boss {
			leash = 22
		}
		// Taunting pets pull attention.
		if e.TauntT > 0 {
			e.TauntT -= dt
		}
		if pet, ok := g.pets[e.PetTarget]; ok && e.TauntT > 0 {
			g.enemyEngage(e, pet.X, pet.Y, speed, m, func(raw float64) { g.damagePet(pet, raw, e) })
			continue
		}
		if e.Target == 0 {
			best := aggroRange * (1 + 0.15*float64(e.Rank-1))
			for pid, p := range g.players {
				if _, safe := g.world.TownAt(p.Ch.X, p.Ch.Y); safe && e.Dungeon < 0 {
					continue
				}
				if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y); d < best && g.world.ClearLine(p.Ch.X, p.Ch.Y, e.X, e.Y) {
					best, e.Target = d, pid
				}
			}
		}
		t, chasing := g.players[e.Target]
		if chasing && dist(e.X, e.Y, e.HomeX, e.HomeY) < leash {
			if _, safe := g.world.TownAt(t.Ch.X, t.Ch.Y); safe && e.Dungeon < 0 {
				e.Target = 0
				continue
			}
			g.enemyEngage(e, t.Ch.X, t.Ch.Y, speed, m, func(raw float64) { g.damagePlayer(t, raw, e) })
			if dist(e.X, e.Y, t.Ch.X, t.Ch.Y) > aggroRange*2.5 {
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
			if rand.Float64() < 0.4 || e.Boss {
				e.WX, e.WY = 0, 0
			} else {
				a := rand.Float64() * 2 * math.Pi
				e.WX, e.WY = math.Cos(a), math.Sin(a)
			}
		}
		s := speed * 0.4 * dt
		g.enemyStep(e, e.WX*s, e.WY*s)
	}
}

// enemyEngage moves toward a target and attacks in melee or at range.
func (g *Game) enemyEngage(e *Enemy, tx, ty, speed float64, m statusMods, hit func(raw float64)) {
	d := dist(e.X, e.Y, tx, ty)
	reach := 0.9 * math.Max(1, e.Scale)
	ranged := e.Fam.Range > 0
	if ranged {
		reach = e.Fam.Range
	}
	clear := g.world.ClearLine(e.X, e.Y, tx, ty)
	if d > reach*0.85 || !clear {
		s := speed * dt / math.Max(d, 0.01)
		g.enemyStep(e, (tx-e.X)*s, (ty-e.Y)*s)
	}
	if d <= reach && clear && e.AttackCD <= 0 {
		e.AttackCD = (1.0 + 0.4*b2f(ranged)) / e.AtkSpeed
		raw := e.Dmg * m.dealt
		if ranged {
			g.fx = append(g.fx, fxBolt(e.X, e.Y, tx, ty, e.Fam.Bolt))
		}
		hit(raw)
	}
}

func b2f(b bool) float64 {
	if b {
		return 1
	}
	return 0
}
