package game

import (
	"math"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

// broadcastState sends each player what is within their view radius.
func (g *Game) broadcastState() {
	for _, p := range g.players {
		c := p.Ch
		near := func(x, y float64) bool {
			return math.Abs(x-c.X) < viewRadius && math.Abs(y-c.Y) < viewRadius
		}
		m := protocol.State{
			T: "state", Players: []protocol.Player{}, Enemies: []protocol.Enemy{},
			Items: []protocol.GroundItem{}, Fx: []protocol.Fx{},
			You: protocol.You{
				ID: p.ID, X: c.X, Y: c.Y, HP: math.Round(c.HP), MaxHP: c.MaxHP(), Level: c.Level,
				XP: c.XP, XPNext: XPForNext(c.Level), Gold: c.Gold,
				Attack: c.Attack(g.pack), Defense: c.Defense(g.pack),
				Inventory: c.Inventory, Weapon: c.Weapon, Armor: c.Armor, Kills: c.Kills, Online: len(g.players),
			},
		}
		for _, o := range g.players {
			if near(o.Ch.X, o.Ch.Y) {
				m.Players = append(m.Players, protocol.Player{
					ID: o.ID, Name: o.Name, X: o.Ch.X, Y: o.Ch.Y, HP: math.Round(o.Ch.HP),
					MaxHP: o.Ch.MaxHP(), Level: o.Ch.Level, Weapon: o.Ch.Weapon, Armor: o.Ch.Armor,
				})
			}
		}
		for id, e := range g.enemies {
			if near(e.X, e.Y) {
				m.Enemies = append(m.Enemies, protocol.Enemy{
					ID: id, Kind: e.Def.ID, X: e.X, Y: e.Y, HP: math.Round(e.HP), MaxHP: e.MaxHP, Level: e.Level,
				})
			}
		}
		for id, it := range g.items {
			if near(it.X, it.Y) {
				m.Items = append(m.Items, protocol.GroundItem{ID: id, Item: it.Item, X: it.X, Y: it.Y})
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
