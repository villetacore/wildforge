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
	S        PStats
	DirX     float64
	DirY     float64
	AttackCD float64
	GatherCD float64
	SinceHit float64
	SpellCD  map[string]float64
	Target   int64
	Pet      *PetEnt
	PetCD    float64
	Dialog   int64
	Zone     int
	invSig   string
	God      bool // dev: ignore damage
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
	ch.normalize(g.pack)
	for _, s := range ch.Inventory {
		if s.G != nil {
			g.refreshGear(s.G)
		}
	}
	for _, s := range ch.Equip {
		if s.G != nil {
			g.refreshGear(s.G)
		}
	}
	if !g.world.Free(ch.X, ch.Y) || (ch.X == 0 && ch.Y == 0) {
		ch.X, ch.Y = g.world.SpawnX, g.world.SpawnY
	}
	p := &Player{ID: ev.id, Name: ev.name, Ch: ch, SinceHit: 99, out: ev.out, SpellCD: map[string]float64{}, Zone: -1}
	p.S = g.computeStats(ch)
	if ch.HP <= 0 {
		ch.HP = p.S.MaxHP
	}
	p.send(protocol.Welcome{
		T: "welcome", ID: p.ID, Name: p.Name, Size: g.world.Size,
		Heights: g.world.Heights, Portals: g.world.Portals, Terrain: g.terrainEnc, Nodes: g.nodesEnc, Zones: g.zonesEnc,
		Depleted: g.world.Depleted(), Structures: g.world.Structures, Towns: g.world.Towns, Dungeons: g.world.Dungeons,
		NPCs: g.npcViews(), Content: g.pack,
	})
	// Dungeons may have reshuffled since the world was encoded.
	for _, d := range g.world.Dungeons {
		p.send(protocol.DungeonPatch{T: "dungeon", Index: d.Index, X: d.X0, Y: d.Y0, Size: d.Size, Terrain: g.world.EncodeRect(d.X0, d.Y0, d.Size), Portals: g.world.Portals})
	}
	p.system(fmt.Sprintf("Welcome to Wildforge, %s! WASD move · right mouse drag rotates the camera · click to target · Space attack · R F G T Z X spells · E talk/interact.", p.Name))
	log.Printf("%s joined", p.Name)
	g.players[p.ID] = p
	g.broadcastSystem(p.Name + " entered the world")
	if ch.ActivePet >= 0 {
		idx := ch.ActivePet
		ch.ActivePet = -1
		g.summonPet(p, idx)
	}
	g.ensureContractTargets(p)
	if !ch.Look.Done {
		g.openMenu(p, "look")
	}
}

func (g *Game) leave(id int64) {
	p, ok := g.players[id]
	if !ok {
		return
	}
	if p.Pet != nil {
		delete(g.pets, p.Pet.ID)
	}
	for eid, e := range g.enemies {
		if e.ContractOwner == id {
			delete(g.enemies, eid)
		}
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
		g.interact(p)
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
	case "cast":
		g.castSpell(p, m.Spell)
	case "target":
		if _, ok := g.enemies[m.Target]; ok || m.Target == 0 {
			p.Target = m.Target
		}
		if n, ok := g.npcs[m.Target]; ok && dist(p.Ch.X, p.Ch.Y, n.X, n.Y) <= talkRange {
			g.talkTo(p, n)
		}
	case "talk":
		g.talk(p, m)
	case "menu":
		g.openMenu(p, m.Menu)
	case "tame":
		g.tame(p)
	case "close":
		p.Dialog = 0
	}
}

// interact (E): portals and caches, then NPCs, then resource nodes.
func (g *Game) interact(p *Player) {
	if g.interactDungeon(p) {
		return
	}
	if n := g.nearestNPC(p, talkRange); n != nil {
		if p.GatherCD <= 0 {
			p.GatherCD = 0.4
			g.talkTo(p, n)
		}
		return
	}
	g.gather(p)
}

func (g *Game) tickPlayers() {
	for _, p := range g.players {
		c := p.Ch
		p.S = g.computeStats(c)
		dot, hot := g.tickStatuses(&c.Statuses)
		if dot > 0 {
			c.HP -= dot
			if c.HP <= 0 {
				c.HP = 1
				g.hurtPlayer(p, 1, "dot")
			}
		}
		if hot > 0 {
			c.HP = math.Min(p.S.MaxHP, c.HP+p.S.MaxHP*hot)
		}
		m := g.mods(c.Statuses)
		if (p.DirX != 0 || p.DirY != 0) && !m.stun {
			sp := p.S.Speed * (1 - m.slow) * dt
			c.X, c.Y = g.world.Step(c.X, c.Y, p.DirX*sp, p.DirY*sp)
			if p.Dialog != 0 {
				if n, ok := g.npcs[p.Dialog]; ok && dist(c.X, c.Y, n.X, n.Y) > talkRange+2 {
					g.closeDialog(p)
				}
			}
		}
		p.AttackCD -= dt
		p.GatherCD -= dt
		p.PetCD -= dt
		p.SinceHit += dt
		for k, v := range p.SpellCD {
			if v -= dt; v <= 0 {
				delete(p.SpellCD, k)
			} else {
				p.SpellCD[k] = v
			}
		}
		if p.SinceHit > 5 || p.S.CombatRegen {
			c.HP = math.Min(c.HP+(p.S.MaxHP*0.02+p.S.Regen)*dt, p.S.MaxHP)
		}
		c.HP = math.Min(c.HP, p.S.MaxHP)
		c.Mana = math.Min(p.S.MaxMana, c.Mana+p.S.ManaRegen*dt*(1+2*b2f(p.SinceHit > 5)))
		c.Mood = math.Max(0, c.Mood-dt/60*(1-p.S.MoodKeep))
		if z := g.world.ZoneOf(c.X, c.Y); z != p.Zone {
			p.Zone = z
			zone := g.pack.Zones[z]
			if !g.world.Underground(c.X, c.Y) {
				p.system(fmt.Sprintf("Entering %s (levels %d–%d).", zone.Name, zone.MinLevel, zone.MaxLevel))
			}
		}
		if _, ok := g.enemies[p.Target]; !ok && p.Target != 0 {
			p.Target = 0
		}
		g.pickupNear(p)
	}
}
