package game

import (
	"encoding/json"
	"testing"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/world"
)

type nopSaver struct{}

func (nopSaver) SaveCharacter(string, json.RawMessage) error { return nil }

func newTestGame(t *testing.T) (*Game, *Player) {
	t.Helper()
	pack, err := content.Load("../../../content")
	if err != nil {
		t.Fatal(err)
	}
	g := New(pack, world.Generate(1337, pack), nopSaver{})
	out := make(chan []byte, 1024)
	g.join(event{kind: evJoin, id: 1, name: "t", out: out})
	return g, g.players[1]
}

func TestInventoryStacksAndLimits(t *testing.T) {
	g, p := newTestGame(t)
	inv := p.Ch.inv(g.pack)
	if left := inv.Add("wood", 150); left != 0 {
		t.Fatalf("left = %d", left)
	}
	if got := inv.Count("wood"); got != 150 {
		t.Fatalf("count = %d", got)
	}
	if !inv.Remove("wood", 120) || inv.Count("wood") != 30 {
		t.Fatalf("remove failed, count = %d", inv.Count("wood"))
	}
	if inv.Remove("wood", 31) {
		t.Fatal("removed more than owned")
	}
}

func TestCraftingConsumesIngredients(t *testing.T) {
	g, p := newTestGame(t)
	inv := p.Ch.inv(g.pack)
	g.craft(p, "wood_sword")
	if inv.Count("wood_sword") != 0 {
		t.Fatal("crafted without ingredients")
	}
	inv.Add("wood", 7)
	g.input(p, protocol.ClientMsg{T: "craft", Recipe: "wood_sword"})
	if inv.Count("wood_sword") != 1 || inv.Count("wood") != 2 {
		t.Fatalf("sword=%d wood=%d", inv.Count("wood_sword"), inv.Count("wood"))
	}
}

func TestGatheringNeedsToolsAndDepletes(t *testing.T) {
	g, p := newTestGame(t)
	w := g.world
	var rockIdx, treeIdx = -1, -1
	isolated := func(i int) bool {
		x, y := i%w.Size, i/w.Size
		for dy := -2; dy <= 2; dy++ {
			for dx := -2; dx <= 2; dx++ {
				j := w.Index(x+dx, y+dy)
				if (dx != 0 || dy != 0) && (w.Nodes[j] != 0 || w.Blocked(x+dx, y+dy)) {
					return false
				}
			}
		}
		return true
	}
	for i, k := range w.Nodes {
		if k == 0 || !isolated(i) {
			continue
		}
		switch g.pack.Resource(k).ID {
		case "rock":
			if rockIdx < 0 {
				rockIdx = i
			}
		case "tree":
			if treeIdx < 0 {
				treeIdx = i
			}
		}
	}
	if rockIdx < 0 || treeIdx < 0 {
		t.Fatal("world has no rocks or trees")
	}
	stand := func(idx int) { p.Ch.X, p.Ch.Y = float64(idx%w.Size)+0.5, float64(idx/w.Size)+1.6; p.GatherCD = 0 }

	stand(rockIdx)
	g.gather(p)
	if p.Ch.inv(g.pack).Count("stone") != 0 {
		t.Fatal("mined stone without a pickaxe")
	}
	stand(treeIdx)
	charges := w.Charges[treeIdx]
	for range charges {
		stand(treeIdx)
		g.gather(p)
	}
	if got := p.Ch.inv(g.pack).Count("wood"); got != charges {
		t.Fatalf("wood = %d, want %d", got, charges)
	}
	if !g.world.Free(float64(treeIdx%w.Size)+0.5, float64(treeIdx/w.Size)+0.5) {
		t.Fatal("depleted tree still blocks movement")
	}
	if _, ok := g.nodeTimers[treeIdx]; !ok {
		t.Fatal("no respawn timer for depleted tree")
	}
}
