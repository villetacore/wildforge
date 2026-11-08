package game

import (
	"encoding/json"
	"fmt"
	"log"
	"math"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

type Player struct {
	ID       int64
	Name     string
	Ch       *Character
	DirX     float64
	DirY     float64
	AttackCD float64
	GatherCD float64
	SinceHit float64
	out      chan []byte
}

func (p *Player) sendRaw(b []byte) {
	select {
	case p.out <- b:
	default: // client too slow: drop rather than stall the world
	}
}

func (p *Player) send(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		log.Printf("marshal: %v", err)
		return
	}
	p.sendRaw(b)
}

func (p *Player) system(text string) { p.send(protocol.System(text)) }

func (g *Game) join(ev event) {
	for _, p := range g.players {
		if p.Name == ev.name {
			b, _ := json.Marshal(protocol.NewError("This character is already online"))
			ev.out <- b
			close(ev.out)
			return
		}
	}
	ch := newCharacter(g.pack)
	if len(ev.character) > 0 {
		var saved Character
		if err := json.Unmarshal(ev.character, &saved); err == nil {
			ch = &saved
		} else {
			log.Printf("%s: saved character unreadable (%v), starting fresh", ev.name, err)
		}
	}
	ch.Level = max(ch.Level, 1)
	if !g.world.Free(ch.X, ch.Y) {
		ch.X, ch.Y = g.world.SpawnX, g.world.SpawnY
	}
	if ch.HP <= 0 {
		ch.HP = ch.MaxHP()
	}
	p := &Player{ID: ev.id, Name: ev.name, Ch: ch, SinceHit: 99, out: ev.out}
	p.send(protocol.Welcome{
		T: "welcome", ID: p.ID, Name: p.Name, Size: g.world.Size,
		Terrain: g.terrainEnc, Nodes: g.nodesEnc, Depleted: g.world.Depleted(), Content: g.pack,
	})
	p.system(fmt.Sprintf("Welcome to Wildforge, %s! WASD move, Space attack, E gather, C craft, Q potion, Enter chat.", p.Name))
	log.Printf("%s joined", p.Name)
	g.players[p.ID] = p
	g.broadcastSystem(p.Name + " entered the world")
}

func (g *Game) leave(id int64) {
	p, ok := g.players[id]
	if !ok {
		return
	}
	delete(g.players, id)
	close(p.out)
	log.Printf("%s left", p.Name)
	g.save(p, true)
	g.broadcastSystem(p.Name + " left the world")
	g.forgetTarget(id)
}

func (g *Game) input(p *Player, m protocol.ClientMsg) {
	switch m.T {
	case "move":
		l := math.Hypot(m.DX, m.DY)
		if l > 0.01 && !math.IsInf(l, 0) && !math.IsNaN(l) {
			p.DirX, p.DirY = m.DX/l, m.DY/l
		} else {
			p.DirX, p.DirY = 0, 0
		}
	case "attack":
		g.playerAttack(p)
	case "gather":
		g.gather(p)
	case "craft":
		g.craft(p, m.Recipe)
	case "chat":
		g.chat(p, m.Text)
	case "use":
		g.useItem(p, m.Index)
	case "potion":
		g.quickPotion(p)
	case "drop":
		g.dropFromInventory(p, m.Index)
	case "unequip":
		g.unequip(p, m.Slot)
	}
}

func (g *Game) tickPlayers() {
	for _, p := range g.players {
		c := p.Ch
		if p.DirX != 0 || p.DirY != 0 {
			c.X, c.Y = g.world.Step(c.X, c.Y, p.DirX*playerSpeed*dt, p.DirY*playerSpeed*dt)
		}
		p.AttackCD -= dt
		p.GatherCD -= dt
		p.SinceHit += dt
		if p.SinceHit > 5 {
			c.HP = math.Min(c.HP+c.MaxHP()*0.03*dt, c.MaxHP())
		}
		g.pickupNear(p)
	}
}
