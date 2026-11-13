package world

import (
	"github.com/villetacore/wildforge/server/internal/content"
	"reflect"
	"testing"
)

func TestDungeonConnectivityAndHeights(t *testing.T) {
	pack, err := content.Load("../../../content")
	if err != nil {
		t.Fatal(err)
	}
	for _, seed := range []uint32{1, 7, 1337, 9999} {
		w := Generate(seed, pack)
		if len(w.Portals) != 4 {
			t.Fatal("expected two paired entrances")
		}
		if !reflect.DeepEqual(w.Terrain, Generate(seed, pack).Terrain) {
			t.Fatal("generation must be deterministic")
		}
		if len(w.Heights) != (w.Size+1)*(w.Size+1) {
			t.Fatal("invalid height grid")
		}
		for i := 0; i < len(w.Portals); i += 2 {
			p := w.Portals[i]
			if !w.Free(p.X, p.Y) || !w.Free(p.ToX, p.ToY) {
				t.Fatal("blocked portal")
			}
			start := w.Index(int(p.ToX), int(p.ToY))
			seen := map[int]bool{start: true}
			queue := []int{start}
			caches := 0
			for len(queue) > 0 {
				idx := queue[0]
				queue = queue[1:]
				if w.Terrain[idx] == Chest {
					caches++
				}
				x, y := idx%w.Size, idx/w.Size
				for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
					xx, yy := x+d[0], y+d[1]
					if w.Blocked(xx, yy) {
						continue
					}
					j := w.Index(xx, yy)
					if !seen[j] {
						seen[j] = true
						queue = append(queue, j)
					}
				}
			}
			if caches == 0 {
				t.Fatal("no reachable loot")
			}
			if len(seen) > 44*44 {
				t.Fatal("dungeon leaks into overworld")
			}
		}
		var max float64
		for _, h := range w.Heights {
			if h > max {
				max = h
			}
		}
		if max < 1 {
			t.Fatal("flat world")
		}
	}
}
