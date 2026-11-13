package game

import (
	"encoding/json"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/storage"
	"github.com/villetacore/wildforge/server/internal/world"
	"path/filepath"
	"testing"
)

func TestDungeonEnterLootExit(t *testing.T) {
	g, p := newTestGame(t)
	link := g.world.Portals[0]
	p.Ch.X, p.Ch.Y = link.X, link.Y
	g.input(p, protocol.ClientMsg{T: "gather"})
	if p.Ch.X != link.ToX || p.Ch.Y != link.ToY {
		t.Fatal("entry failed")
	}
	p.GatherCD = 0
	g.input(p, protocol.ClientMsg{T: "gather"})
	if p.Ch.X != link.X || p.Ch.Y != link.Y {
		t.Fatal("exit failed")
	}
	for idx, code := range g.world.Terrain {
		if code != world.Chest {
			continue
		}
		p.Ch.X, p.Ch.Y = float64(idx%g.world.Size)+.5, float64(idx/g.world.Size)+.5
		before := p.Ch.Gold
		p.GatherCD = 0
		g.input(p, protocol.ClientMsg{T: "gather"})
		if p.Ch.Gold <= before {
			t.Fatal("cache produced no loot")
		}
		after := p.Ch.Gold
		p.GatherCD = 0
		g.input(p, protocol.ClientMsg{T: "gather"})
		if p.Ch.Gold != after {
			t.Fatal("cache duplicated loot")
		}
		return
	}
	t.Fatal("no caches")
}

func TestDungeonLootSharedAndFullInventory(t *testing.T) {
	g, p := newTestGame(t)
	for idx, code := range g.world.Terrain {
		if code != world.Chest {
			continue
		}
		p.Ch.X, p.Ch.Y = float64(idx%g.world.Size)+.5, float64(idx/g.world.Size)+.5
		// A zero capacity inventory cannot consume the cache.
		p.Ch.Inventory = nil
		capacity := g.pack.World.MaxInventory
		g.pack.World.MaxInventory = 0
		before := p.Ch.Gold
		g.interactDungeon(p)
		if p.Ch.Gold != before || g.chestTimers[idx] > 0 {
			t.Fatal("full inventory consumed cache")
		}
		g.pack.World.MaxInventory = capacity
		p.GatherCD = 0
		g.interactDungeon(p)
		second := &Player{Ch: newCharacter(g.pack), out: make(chan []byte, 10)}
		second.Ch.X, second.Ch.Y = p.Ch.X, p.Ch.Y
		g.interactDungeon(second)
		if second.Ch.Gold != 0 {
			t.Fatal("shared cache gave duplicate rewards")
		}
		return
	}
	t.Fatal("no cache")
}

func TestDungeonGuardRespawnsAtHome(t *testing.T) {
	g, p := newTestGame(t)
	for id, e := range g.enemies {
		if !e.Dungeon {
			continue
		}
		delete(g.enemies, id)
		e.HP = 0
		g.guardTimers[e] = 0
		p.Ch.X, p.Ch.Y = e.HomeX, e.HomeY
		g.tickRespawns()
		if len(g.guardTimers) != 1 {
			t.Fatal("respawn beside player")
		}
		p.Ch.X, p.Ch.Y = g.world.SpawnX, g.world.SpawnY
		g.guardTimers[e] = 0
		g.tickRespawns()
		if len(g.guardTimers) != 0 || e.HP != e.MaxHP || e.X != e.HomeX || e.Y != e.HomeY {
			t.Fatal("guard respawn failed")
		}
		return
	}
	t.Fatal("missing guardians")
}

func TestDungeonLootPersists(t *testing.T) {
	g, p := newTestGame(t)
	path := filepath.Join(t.TempDir(), "users.json")
	store, err := storage.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Register(p.Name, "test-password"); err != nil {
		t.Fatal(err)
	}
	g.saver = store
	for idx, code := range g.world.Terrain {
		if code != world.Chest {
			continue
		}
		p.Ch.X, p.Ch.Y = float64(idx%g.world.Size)+.5, float64(idx/g.world.Size)+.5
		g.interactDungeon(p)
		g.save(p, false)
		reopened, err := storage.Open(path)
		if err != nil {
			t.Fatal(err)
		}
		data, err := reopened.Authenticate(p.Name, "test-password")
		if err != nil {
			t.Fatal(err)
		}
		var saved Character
		if err := json.Unmarshal(data, &saved); err != nil {
			t.Fatal(err)
		}
		if saved.Gold != p.Ch.Gold || saved.Gold == 0 || saved.inv(g.pack).Count("potion") != p.Ch.inv(g.pack).Count("potion") {
			t.Fatal("loot was not persisted")
		}
		return
	}
	t.Fatal("no cache")
}
