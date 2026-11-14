package game

import (
	"encoding/json"
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

func (g *Game) playerView(o *Player) protocol.Player {
	c := o.Ch
	v := protocol.Player{
		ID: o.ID, Name: o.Name, Title: c.Title, X: c.X, Y: c.Y, HP: math.Round(c.HP),
		MaxHP: math.Round(o.S.MaxHP), Level: c.Level, Look: c.Look,
	}
	if s := c.Equip["weapon"]; s != nil {
		v.Weapon, v.WColor = "sword", "#d0d4dc"
		if s.G != nil {
			v.Weapon, v.WColor = s.G.Model, s.G.Tint
		} else if d := g.pack.Item(s.Item); d != nil {
			v.WColor = d.Color
		}
	}
	if s := c.Equip["chest"]; s != nil {
		if s.G != nil {
			v.Armor = s.G.Tint
		} else if d := g.pack.Item(s.Item); d != nil {
			v.Armor = d.Color
		}
	}
	if s := c.Equip["head"]; s != nil && s.G != nil {
		v.Helm = s.G.Tint
	}
	if s := c.Equip["offhand"]; s != nil && s.G != nil {
		v.Shield = s.G.Model + ":" + s.G.Tint
	}
	return v
}

// broadcastState sends each player what is within their view radius.
func (g *Game) broadcastState() {
	tod := g.TimeOfDay()
	for _, p := range g.players {
		c := p.Ch
		near := func(x, y float64) bool {
			return math.Abs(x-c.X) < viewRadius && math.Abs(y-c.Y) < viewRadius
		}
		zone := g.pack.Zones[g.world.ZoneOf(c.X, c.Y)].Name
		if d, ok := g.world.DungeonAt(c.X, c.Y); ok {
			zone = d.Name
		} else if t, ok := g.world.TownAt(c.X, c.Y); ok {
			zone = g.world.Towns[t].Name + " · " + zone
		}
		m := protocol.State{
			T: "state", Players: []protocol.Player{}, Enemies: []protocol.Enemy{}, Pets: g.petViews(near),
			Items: []protocol.GroundItem{}, Fx: []protocol.Fx{},
			You: protocol.You{
				ID: p.ID, X: c.X, Y: c.Y, HP: math.Round(c.HP), MaxHP: math.Round(p.S.MaxHP), Mana: math.Round(c.Mana), MaxMana: math.Round(p.S.MaxMana),
				Level: c.Level, XP: c.XP, XPNext: XPForNext(c.Level), Gold: c.Gold,
				Attack: int(p.S.Atk), Defense: int(p.S.Def), SP: int(p.S.SP), Crit: math.Round(p.S.Crit*10) / 10,
				Attrs: c.Attrs, StatPoints: c.StatPoints, PerkPoints: c.PerkPoints, Mood: math.Round(c.Mood),
				Kills: c.Kills, Online: len(g.players),
				Statuses: g.statusViews(c.Statuses), Spells: g.spellBar(p), Zone: zone, Target: p.Target,
				Marks: g.contractMarks(p), Title: c.Title, TOD: math.Round(tod*1000) / 1000,
			},
		}
		if sig, _ := json.Marshal([]any{c.Inventory, c.Equip}); string(sig) != p.invSig {
			p.invSig = string(sig)
			inv := c.Inventory
			if inv == nil {
				inv = []content.Stack{}
			}
			m.You.Inventory = &inv
			m.You.Equip = c.Equip
		}
		if s := c.Equip["weapon"]; s != nil {
			m.You.Weapon = g.stackName(s)
		}
		if s := c.Equip["chest"]; s != nil {
			m.You.Armor = g.stackName(s)
		}
		for _, o := range g.players {
			if near(o.Ch.X, o.Ch.Y) {
				m.Players = append(m.Players, g.playerView(o))
			}
		}
		for id, e := range g.enemies {
			if near(e.X, e.Y) {
				st := make([]string, 0, len(e.Statuses))
				for _, s := range e.Statuses {
					st = append(st, s.ID)
				}
				m.Enemies = append(m.Enemies, protocol.Enemy{
					ID: id, Kind: e.Fam.ID, Name: e.Name, Model: e.Fam.Model, Tex: e.Fam.Texture, Color: e.Color, Glow: e.Glow,
					Scale: e.Scale, Rank: e.Rank, X: e.X, Y: e.Y, HP: math.Round(e.HP), MaxHP: e.MaxHP, Level: e.Level, Status: st,
				})
			}
		}
		for id, it := range g.items {
			if near(it.X, it.Y) {
				gi := protocol.GroundItem{ID: id, Item: it.Item, X: it.X, Y: it.Y}
				if it.G != nil {
					gi.Color = it.G.Color
				}
				m.Items = append(m.Items, gi)
			}
		}
		for _, f := range g.fx {
			if near(f.X, f.Y) {
				m.Fx = append(m.Fx, f)
			}
		}
		p.send(m)
	}
}
