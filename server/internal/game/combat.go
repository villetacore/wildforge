package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

func (g *Game) playerAttack(p *Player) {
	if p.AttackCD > 0 {
		return
	}
	p.AttackCD = attackCooldown
	g.addFx("swing", p.Ch.X, p.Ch.Y, "")

	var targetID int64
	best := attackRange
	for id, e := range g.enemies {
		if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y) - 0.3*(e.Def.Scale-1); d <= best {
			best, targetID = d, id
		}
	}
	e, ok := g.enemies[targetID]
	if !ok {
		return
	}
	dmg := math.Max(1, math.Round(float64(p.Ch.Attack(g.pack))*randRange(0.8, 1.2)))
	text := fmt.Sprint(dmg)
	if rand.Float64() < 0.1 {
		dmg *= 2
		text = fmt.Sprintf("%v!", dmg)
	}
	e.HP -= dmg
	e.Target = p.ID
	g.addFx("hit", e.X, e.Y, text)
	if e.HP <= 0 {
		delete(g.enemies, targetID)
		g.enemyTimers = append(g.enemyTimers, g.pack.World.EnemyRespawn)
		g.onEnemyKilled(p, e)
	}
}

func (g *Game) onEnemyKilled(p *Player, e *Enemy) {
	c := p.Ch
	xp := e.Def.XP * e.Level
	c.Kills++
	c.XP += xp
	g.addFx("xp", c.X, c.Y, fmt.Sprintf("+%d XP", xp))
	leveled := false
	for c.XP >= XPForNext(c.Level) {
		c.XP -= XPForNext(c.Level)
		c.Level++
		leveled = true
	}
	if leveled {
		c.HP = c.MaxHP()
		g.addFx("level", c.X, c.Y, fmt.Sprintf("LEVEL %d!", c.Level))
		g.broadcastSystem(fmt.Sprintf("%s reached level %d!", p.Name, c.Level))
	}
	gp := g.pack.World.GoldPerLevel
	g.dropOnGround("gold", e.Level*randInt(gp[0], gp[1]), e.X, e.Y)
	for _, d := range e.Def.Drops {
		if rand.Float64() < d.Chance {
			g.dropOnGround(d.Item, randInt(d.Min, d.Max), e.X, e.Y)
		}
	}
}

func (g *Game) enemyHit(p *Player, raw float64) {
	c := p.Ch
	dmg := math.Max(1, math.Round(raw*randRange(0.8, 1.2)-float64(c.Defense(g.pack))))
	c.HP -= dmg
	p.SinceHit = 0
	g.addFx("hurt", c.X, c.Y, fmt.Sprintf("-%d", int(dmg)))
	if c.HP > 0 {
		return
	}
	lost := c.Gold / 10
	c.Gold -= lost
	c.HP = c.MaxHP()
	c.X, c.Y = g.world.SpawnX, g.world.SpawnY
	p.DirX, p.DirY = 0, 0
	p.send(protocol.Died{T: "died", LostGold: lost})
	g.forgetTarget(p.ID)
	g.broadcastSystem(p.Name + " has fallen")
}
