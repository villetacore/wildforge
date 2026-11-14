package world

import (
	"reflect"
	"testing"

	"github.com/villetacore/wildforge/server/internal/content"
)

func loadPack(t *testing.T) *content.Pack {
	t.Helper()
	pack, err := content.Load("../../../content")
	if err != nil {
		t.Fatal(err)
	}
	return pack
}

// reachable floods walkable tiles from (x, y) and counts caches.
func reachable(w *World, x, y int, limit int) (int, int) {
	start := w.Index(x, y)
	seen := map[int]bool{start: true}
	queue := []int{start}
	caches := 0
	for len(queue) > 0 && len(seen) <= limit {
		idx := queue[0]
		queue = queue[1:]
		if w.Terrain[idx] == Chest {
			caches++
		}
		cx, cy := idx%w.Size, idx/w.Size
		for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
			xx, yy := cx+d[0], cy+d[1]
			if w.Blocked(xx, yy) && !(w.InBounds(xx, yy) && w.Terrain[w.Index(xx, yy)] == Chest) {
				continue
			}
			j := w.Index(xx, yy)
			if !seen[j] {
				seen[j] = true
				if w.Terrain[j] != Chest {
					queue = append(queue, j)
				} else {
					caches++
				}
			}
		}
	}
	return len(seen), caches
}

func TestDungeonsConnectedAndRegenerate(t *testing.T) {
	pack := loadPack(t)
	for _, seed := range []uint32{1, 7, 1337} {
		w := Generate(seed, pack)
		if len(w.Dungeons) != len(pack.Zones) || len(w.Portals) != 2*len(w.Dungeons) {
			t.Fatalf("dungeons=%d portals=%d", len(w.Dungeons), len(w.Portals))
		}
		if !reflect.DeepEqual(w.Terrain, Generate(seed, pack).Terrain) {
			t.Fatal("generation must be deterministic")
		}
		if len(w.Heights) != (w.Size+1)*(w.Size+1) {
			t.Fatal("invalid height grid")
		}
		for i, d := range w.Dungeons {
			for gen := 0; gen < 3; gen++ {
				in := w.Portals[2*i]
				if !w.Free(in.X, in.Y) || !w.Free(in.ToX, in.ToY) {
					t.Fatalf("dungeon %d gen %d: blocked portal", i, d.Gen)
				}
				if !d.Contains(in.ToX, in.ToY) {
					t.Fatal("portal does not lead into the dungeon")
				}
				size, caches := reachable(w, int(in.ToX), int(in.ToY), DungeonSize*DungeonSize+1)
				if size > DungeonSize*DungeonSize {
					t.Fatal("dungeon leaks into overworld")
				}
				if caches == 0 {
					t.Fatalf("dungeon %d gen %d: no reachable loot", i, d.Gen)
				}
				// Every spawn point, especially the boss, is reachable from the entrance.
				dist := w.floodDistance(int(in.ToX), int(in.ToY), d)
				boss := false
				for _, s := range d.Spawns {
					if _, ok := dist[w.Index(s[0], s[1])]; !ok {
						t.Fatalf("dungeon %d gen %d: unreachable spawn %v", i, d.Gen, s)
					}
					boss = boss || s[2] == 1
				}
				if !boss {
					t.Fatal("no boss")
				}
				before := w.EncodeRect(d.X0, d.Y0, d.Size)
				w.RegenDungeon(i, seed+uint32(gen)*31+99)
				if w.EncodeRect(d.X0, d.Y0, d.Size) == before {
					t.Fatal("regeneration produced the same layout")
				}
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

func TestTownsZonesAndCapital(t *testing.T) {
	pack := loadPack(t)
	w := Generate(1337, pack)
	if len(w.Towns) != len(pack.Zones) {
		t.Fatalf("towns = %d", len(w.Towns))
	}
	if !w.Free(w.SpawnX, w.SpawnY) {
		t.Fatal("spawn blocked")
	}
	kinds := map[string]int{}
	for _, s := range w.Structures {
		kinds[s.Kind]++
	}
	for _, k := range []string{"tavern", "smithy", "townhall", "church", "cathedral", "palace", "magetower", "kings_bridge", "house"} {
		if kinds[k] == 0 {
			t.Errorf("capital is missing a %s (have %v)", k, kinds)
		}
	}
	if kinds["house"] < 40 {
		t.Errorf("capital should be dense, only %d houses", kinds["house"])
	}
	roles := map[string]int{}
	for _, s := range w.NPCSpots {
		roles[s.Role]++
		if !w.Free(s.X, s.Y) {
			t.Errorf("%s stands inside a wall at %.1f,%.1f", s.Role, s.X, s.Y)
		}
	}
	for _, r := range []string{"innkeeper", "board", "courtesan", "blacksmith", "merchant", "healer", "enchanter", "trainer", "barber", "stablemaster", "guard", "king", "exotic"} {
		if roles[r] == 0 {
			t.Errorf("no %s placed (have %v)", r, roles)
		}
	}
	if roles["courtesan"] < 3 {
		t.Error("the Drunken Griffin needs its three ladies")
	}
	// Every town is reachable from the capital on foot.
	size, _ := reachable(w, int(w.SpawnX), int(w.SpawnY), w.Size*w.Size)
	_ = size
	for _, town := range w.Towns[1:] {
		seen := map[int]bool{}
		start := w.Index(int(w.SpawnX), int(w.SpawnY))
		seen[start] = true
		queue := []int{start}
		found := false
		goal := w.Index(int(town.X), int(town.Y))
		for len(queue) > 0 && !found {
			idx := queue[0]
			queue = queue[1:]
			if idx == goal || (w.Terrain[idx] == Cobble && abs(idx%w.Size-int(town.X)) < 4 && abs(idx/w.Size-int(town.Y)) < 4) {
				found = true
			}
			x, y := idx%w.Size, idx/w.Size
			for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
				xx, yy := x+d[0], y+d[1]
				if w.InBounds(xx, yy) && !w.Blocked(xx, yy) || (w.InBounds(xx, yy) && w.Nodes[w.Index(xx, yy)] != 0 && !Solid(w.Terrain[w.Index(xx, yy)]) && w.Terrain[w.Index(xx, yy)] != Water && w.Terrain[w.Index(xx, yy)] != Lava) {
					j := w.Index(xx, yy)
					if !seen[j] {
						seen[j] = true
						queue = append(queue, j)
					}
				}
			}
		}
		if !found {
			t.Errorf("%s is not reachable from Velhrad", town.Name)
		}
	}
	zones := map[uint8]bool{}
	for _, z := range w.Zone {
		zones[z] = true
	}
	if len(zones) != len(pack.Zones) {
		t.Fatalf("only %d zones on the map", len(zones))
	}
}
