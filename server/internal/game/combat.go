package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

func fxBolt(x, y, x2, y2 float64, color string) protocol.Fx {
	return protocol.Fx{Kind: "bolt", X: x, Y: y, X2: x2, Y2: y2, Color: color}
}

// pickTarget returns the player's selected enemy if valid and in range,
// otherwise the nearest visible enemy within range.
func (g *Game) pickTarget(p *Player, rng float64) (int64, *Enemy) {
	if e, ok := g.enemies[p.Target]; ok {
		if dist(p.Ch.X, p.Ch.Y, e.X, e.Y)-0.3*(e.Scale-1) <= rng && g.world.ClearLine(p.Ch.X, p.Ch.Y, e.X, e.Y) {
			return p.Target, e
		}
	}
	var best int64
	bd := rng
	for id, e := range g.enemies {
		if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y) - 0.3*(e.Scale-1); d <= bd && g.world.ClearLine(p.Ch.X, p.Ch.Y, e.X, e.Y) {
			bd, best = d, id
		}
	}
	return best, g.enemies[best]
}

func (g *Game) playerAttack(p *Player) {
	if p.AttackCD > 0 || g.mods(p.Ch.Statuses).stun {
		return
	}
	p.AttackCD = attackCooldown
	g.addFx("swing", p.Ch.X, p.Ch.Y, "")
	id, e := g.pickTarget(p, attackRange)
	if e == nil {
		return
	}
	p.Target = id
	s := &p.S
	dmg := s.Atk * randRange(0.85, 1.15) * g.mods(p.Ch.Statuses).dealt
	if s.Opener > 0 && e.HP >= e.MaxHP {
		dmg *= 1 + s.Opener
	}
	crit := rand.Float64()*100 < s.Crit
	if crit {
		dmg *= s.CritMult
	}
	g.damageEnemy(p, id, e, dmg, "phys", crit, true)
	// Elemental riders from gear.
	if _, alive := g.enemies[id]; alive {
		for _, el := range elements {
			if v := s.Elem[el]; v > 0 {
				g.damageEnemy(p, id, e, v*randRange(0.8, 1.2), el, false, false)
				if _, alive := g.enemies[id]; !alive {
					break
				}
			}
		}
	}
	if _, alive := g.enemies[id]; alive && s.PoisonHit > 0 && rand.Float64() < s.PoisonHit {
		g.addStatus(&e.Statuses, "poison", 1+float64(p.Ch.Level)*0.3)
	}
	// Cleave hits extra enemies near the target for half damage.
	if s.Cleave > 0 {
		n := 0
		for oid, o := range g.enemies {
			if oid == id || n >= s.Cleave || dist(o.X, o.Y, e.X, e.Y) > 2 || dist(o.X, o.Y, p.Ch.X, p.Ch.Y) > attackRange+1 {
				continue
			}
			n++
			g.damageEnemy(p, oid, o, s.Atk*0.5, "phys", false, true)
		}
	}
}

// resist returns the damage multiplier of an enemy against a school.
func resist(e *Enemy, school string) float64 {
	if school == "phys" {
		return 1
	}
	if e.Elem.ID == school {
		return 0.5
	}
	opp := map[string]string{"fire": "frost", "frost": "fire", "holy": "shadow", "shadow": "holy", "storm": "stone", "poison": "holy"}
	if opp[school] == e.Elem.ID {
		return 1.35
	}
	if school == "holy" && (e.Fam.ID == "skeleton" || e.Fam.ID == "ghoul") {
		return 1.6
	}
	return 1
}

// damageEnemy applies player-caused damage, handling dodge, armor,
// resistances, thorns, lifesteal and death.
func (g *Game) damageEnemy(p *Player, id int64, e *Enemy, raw float64, school string, crit, melee bool) {
	if melee && e.Dodge > 0 && rand.Float64() < e.Dodge {
		g.addFx("hit", e.X, e.Y, "dodge")
		return
	}
	dmg := raw * resist(e, school) * g.mods(e.Statuses).taken
	if school == "phys" {
		dmg -= e.Armor
	}
	dmg = math.Max(1, math.Round(dmg))
	e.HP -= dmg
	e.Target = p.ID
	e.LastHit = p.ID
	text := fmt.Sprint(dmg)
	if crit {
		text += "!"
	}
	kind := "hit"
	if school != "phys" {
		kind = "hit_" + school
	}
	g.addFx(kind, e.X, e.Y, text)
	if melee {
		if p.S.Lifesteal > 0 {
			g.healPlayer(p, dmg*p.S.Lifesteal, false)
		}
		if e.Thorns > 0 {
			g.hurtPlayer(p, dmg*e.Thorns, "thorns")
		}
	}
	if e.HP <= 0 {
		g.killEnemy(p, id, e)
	}
}

func (g *Game) killEnemy(p *Player, id int64, e *Enemy) {
	if _, ok := g.enemies[id]; !ok {
		return
	}
	delete(g.enemies, id)
	switch {
	case e.Dungeon >= 0:
		if e.Boss {
			g.onBossKilled(p, e)
		}
	case e.Respawns:
		g.enemyTimers = append(g.enemyTimers, g.pack.World.EnemyRespawn)
	}
	if e.Explode > 0 {
		g.addFx("nova", e.X, e.Y, "")
		for _, o := range g.players {
			if dist(o.Ch.X, o.Ch.Y, e.X, e.Y) < 2.2 {
				g.hurtPlayer(o, e.Dmg*e.Explode, "explosion")
			}
		}
	}
	g.onEnemyKilled(p, e)
}

func (g *Game) onEnemyKilled(p *Player, e *Enemy) {
	c := p.Ch
	c.Kills++
	xp := int(e.XP * float64(e.Level) * (1 + p.S.XP))
	// Gray monsters far below your level give little experience.
	if diff := c.Level - e.Level; diff > 4 {
		xp = max(1, xp/(diff-2))
	}
	g.giveXP(p, xp)
	g.petGainXP(p, xp/2)
	gp := g.pack.World.GoldPerLevel
	gold := float64(e.Level*randInt(gp[0], gp[1])) * e.GoldMult * (1 + p.S.Gold) * math.Max(1, g.rankDef(e.Rank).Loot*0.6)
	g.dropOnGround(content.Stack{Item: "gold", Qty: int(gold)}, e.X, e.Y)
	for _, d := range e.Fam.Drops {
		if rand.Float64() < d.Chance*(1+p.S.MF*0.5) {
			g.dropOnGround(content.Stack{Item: d.Item, Qty: randInt(d.Min, d.Max)}, e.X, e.Y)
		}
	}
	if e.Elem.Drop != "" && rand.Float64() < 0.18*e.Loot {
		g.dropOnGround(content.Stack{Item: e.Elem.Drop, Qty: 1}, e.X, e.Y)
	}
	// Generated gear: rarer with rank and magic find.
	chance := 0.07 * e.Loot * (1 + p.S.MF)
	for chance > 0 {
		if rand.Float64() < chance {
			minR := 0
			if e.Boss {
				minR = 2
			}
			gear := g.genGear(e.Level, GearSpec{Quality: -1, Rarity: -1, MinRarity: minR, RarityBoost: math.Log1p(e.Loot) + p.S.MF})
			g.dropOnGround(content.Stack{Item: "gear:" + gear.Base, Qty: 1, G: gear}, e.X, e.Y)
		}
		chance -= 1
	}
	// Reputation: killing faction members angers them and pleases their enemies.
	if f := g.pack.Faction(e.Fam.Faction); f != nil {
		g.changeRep(p, f.ID, -25)
		for _, en := range f.Enemies {
			g.changeRep(p, en, 10)
		}
	}
	g.contractKill(p, e)
}

func (g *Game) giveXP(p *Player, xp int) {
	c := p.Ch
	c.XP += xp
	g.addFx("xp", c.X, c.Y, fmt.Sprintf("+%d XP", xp))
	leveled := false
	for c.XP >= XPForNext(c.Level) {
		c.XP -= XPForNext(c.Level)
		c.Level++
		c.StatPoints += 2
		c.PerkPoints++
		leveled = true
	}
	if leveled {
		p.S = g.computeStats(c)
		c.HP = p.S.MaxHP
		c.Mana = p.S.MaxMana
		g.addFx("level", c.X, c.Y, fmt.Sprintf("LEVEL %d!", c.Level))
		g.broadcastSystem(fmt.Sprintf("%s reached level %d!", p.Name, c.Level))
		p.system("You gained 2 attribute points and a perk point. Press K to spend them.")
	}
}

func (g *Game) healPlayer(p *Player, amount float64, show bool) {
	c := p.Ch
	before := c.HP
	c.HP = math.Min(c.HP+amount, p.S.MaxHP)
	if show && c.HP-before >= 1 {
		g.addFx("heal", c.X, c.Y, fmt.Sprintf("+%d", int(c.HP-before)))
	}
}

// damagePlayer applies an enemy hit: dodge, armor, shield, statuses, thorns.
func (g *Game) damagePlayer(p *Player, raw float64, src *Enemy) {
	c := p.Ch
	if raw = g.defend(p, raw, src); raw <= 0 {
		return
	}
	if rand.Float64() < p.S.Dodge {
		g.addFx("hurt", c.X, c.Y, "dodge")
		return
	}
	dmg := math.Max(1, math.Round(raw*randRange(0.85, 1.15)-p.S.Def))
	if src != nil {
		if src.Lifesteal > 0 {
			src.HP = math.Min(src.MaxHP, src.HP+dmg*src.Lifesteal)
		}
		for _, h := range src.OnHit {
			if rand.Float64() < h.chance {
				def := g.pack.Status(h.status)
				if def != nil && def.Disease && rand.Float64() < p.S.DiseaseResist {
					continue
				}
				if g.addStatus(&c.Statuses, h.status, 1+float64(src.Level)*0.35) && def != nil && def.Disease {
					p.system("You caught " + def.Name + "! " + def.Desc)
				}
			}
		}
		if p.S.Thorns > 0 {
			for id, e := range g.enemies {
				if e == src {
					g.damageEnemy(p, id, e, p.S.Thorns, "phys", false, false)
					break
				}
			}
		}
	}
	g.hurtPlayer(p, dmg, "")
}

// hurtPlayer removes health (after mitigation) and handles death.
func (g *Game) hurtPlayer(p *Player, dmg float64, _ string) {
	c := p.Ch
	if p.God {
		return
	}
	dmg = math.Round(g.absorb(&c.Statuses, dmg*g.mods(c.Statuses).taken))
	if dmg <= 0 {
		g.addFx("hurt", c.X, c.Y, "absorbed")
		return
	}
	c.HP -= dmg
	p.SinceHit = 0
	g.addFx("hurt", c.X, c.Y, fmt.Sprintf("-%d", int(dmg)))
	if c.HP > 0 {
		return
	}
	lost := c.Gold / 10
	if p.S.NoDeathPenalty {
		lost = 0
	}
	c.Gold -= lost
	c.HP = p.S.MaxHP
	c.Mana = p.S.MaxMana
	c.Mood = math.Max(0, c.Mood-15)
	removeStatuses(&c.Statuses, func(s Status) bool { return s.T < 0 })
	c.X, c.Y = g.world.SpawnX, g.world.SpawnY
	p.DirX, p.DirY = 0, 0
	p.send(protocol.Died{T: "died", LostGold: lost})
	g.forgetTarget(p.ID)
	g.broadcastSystem(p.Name + " has fallen")
}
