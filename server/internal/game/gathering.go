package game

import (
	"fmt"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

const nodeRetry = 5.0 // seconds to wait if something stands on a respawning node

// gather harvests the nearest resource node. Tools of the right kind raise
// the yield; some nodes need a minimum tool tier.
func (g *Game) gather(p *Player) {
	if p.GatherCD > 0 {
		return
	}
	p.GatherCD = gatherCooldown
	w := g.world
	idx, ok := w.NearestNode(p.Ch.X, p.Ch.Y, gatherRange)
	if !ok {
		return
	}
	res := g.pack.Resource(w.Nodes[idx])
	nx, ny := float64(idx%w.Size)+0.5, float64(idx/w.Size)+0.5
	inv := p.Ch.inv(g.pack)
	tier := inv.ToolTier(res.Tool)
	if tier < res.MinTier {
		p.system(fmt.Sprintf("You need a %s (tier %d) to gather %s.", res.Tool, res.MinTier, res.Name))
		g.addFx("swing", p.Ch.X, p.Ch.Y, "")
		return
	}
	yield := min(1+tier-res.MinTier, 3)
	left := inv.Add(res.Item, yield)
	if left == yield {
		p.system("Inventory full.")
		return
	}
	name := res.Item
	if it := g.pack.Item(res.Item); it != nil {
		name = it.Name
	}
	g.addFx("gather", nx, ny, fmt.Sprintf("+%d %s", yield-left, name))
	w.Charges[idx]--
	if w.Charges[idx] <= 0 {
		g.nodeTimers[idx] = res.Respawn
		g.broadcast(protocol.Node{T: "node", Index: idx, Alive: false})
	}
}

// tickNodes respawns depleted nodes once their timer runs out and the tile is clear.
func (g *Game) tickNodes() {
	w := g.world
	for idx, t := range g.nodeTimers {
		if t -= dt; t > 0 {
			g.nodeTimers[idx] = t
			continue
		}
		nx, ny := float64(idx%w.Size)+0.5, float64(idx/w.Size)+0.5
		if g.occupied(nx, ny) {
			g.nodeTimers[idx] = nodeRetry
			continue
		}
		delete(g.nodeTimers, idx)
		w.Charges[idx] = g.pack.Resource(w.Nodes[idx]).Charges
		g.broadcast(protocol.Node{T: "node", Index: idx, Alive: true})
	}
}

func (g *Game) occupied(x, y float64) bool {
	for _, p := range g.players {
		if dist(p.Ch.X, p.Ch.Y, x, y) < 1.0 {
			return true
		}
	}
	for _, e := range g.enemies {
		if dist(e.X, e.Y, x, y) < 1.0 {
			return true
		}
	}
	return false
}
