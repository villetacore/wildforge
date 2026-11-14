package game

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/storage"
	"github.com/villetacore/wildforge/server/internal/world"
)

type nopSaver struct{}

func (nopSaver) SaveCharacter(string, json.RawMessage) error { return nil }

var (
	testPack  *content.Pack
	testWorld *world.World
)

func newTestGame(t *testing.T) (*Game, *Player) {
	t.Helper()
	if testPack == nil {
		pack, err := content.Load("../../../content")
		if err != nil {
			t.Fatal(err)
		}
		testPack = pack
	}
	// The world is regenerated per test: tests mutate terrain (dungeons).
	g := New(testPack, world.Generate(1337, testPack), nopSaver{})
	out := make(chan []byte, 4096)
	g.join(event{kind: evJoin, id: 1, name: "t", out: out})
	return g, g.players[1]
}

// drain empties a player's outgoing messages and returns the dialogs seen.
func drain(p *Player) []protocol.Dialog {
	var out []protocol.Dialog
	for {
		select {
		case b := <-p.out:
			if strings.Contains(string(b[:min(len(b), 20)]), `"dialog"`) {
				var d protocol.Dialog
				json.Unmarshal(b, &d)
				out = append(out, d)
			}
		default:
			return out
		}
	}
}

func lastDialog(t *testing.T, p *Player) protocol.Dialog {
	t.Helper()
	ds := drain(p)
	if len(ds) == 0 {
		t.Fatal("no dialog")
	}
	return ds[len(ds)-1]
}

func hasOption(d protocol.Dialog, prefix string) (protocol.Option, bool) {
	for _, o := range d.Options {
		if strings.HasPrefix(o.ID, prefix) {
			return o, true
		}
	}
	return protocol.Option{}, false
}

func npcOfRole(g *Game, role string) *NPC {
	for _, n := range g.npcs {
		if n.Role.ID == role && n.Town == 0 {
			return n
		}
	}
	return nil
}

func standAt(p *Player, n *NPC) { p.Ch.X, p.Ch.Y = n.X, n.Y+0.5 }

func TestInventoryStacksAndLimits(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.Inventory = nil
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
	p.Ch.Inventory = nil
	inv := p.Ch.inv(g.pack)
	g.craft(p, "wood_pickaxe")
	if inv.Count("wood_pickaxe") != 0 {
		t.Fatal("crafted without ingredients")
	}
	inv.Add("wood", 7)
	g.input(p, protocol.ClientMsg{T: "craft", Recipe: "wood_pickaxe"})
	if inv.Count("wood_pickaxe") != 1 || inv.Count("wood") != 1 {
		t.Fatalf("pickaxe=%d wood=%d", inv.Count("wood_pickaxe"), inv.Count("wood"))
	}
}

func TestGatheringNeedsToolsAndDepletes(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.Inventory = nil
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

func TestEnemyGenerationIsVaried(t *testing.T) {
	g, _ := newTestGame(t)
	names := map[string]bool{}
	families := map[string]bool{}
	elems := map[string]bool{}
	ranks := map[int]bool{}
	for _, e := range g.enemies {
		names[e.Name] = true
		families[e.Fam.ID] = true
		elems[e.Elem.ID] = true
		ranks[e.Rank] = true
		if e.MaxHP <= 0 || e.Dmg <= 0 || e.Speed <= 0 {
			t.Fatalf("broken enemy %s: %+v", e.Name, e)
		}
		if _, safe := g.world.TownAt(e.X, e.Y); safe && e.Dungeon < 0 {
			t.Fatalf("%s spawned inside a town", e.Name)
		}
	}
	if len(names) < 200 || len(families) < 12 || len(elems) < 7 || len(ranks) < 5 {
		t.Fatalf("not enough variety: %d names, %d families, %d elements, %d ranks", len(names), len(families), len(elems), len(ranks))
	}
	bosses := 0
	for _, e := range g.enemies {
		if e.Boss {
			bosses++
		}
	}
	if bosses != len(g.world.Dungeons) {
		t.Fatalf("bosses = %d", bosses)
	}
}

func TestGearGenerationAndEquip(t *testing.T) {
	g, p := newTestGame(t)
	names := map[string]bool{}
	rarities := map[int]bool{}
	for i := 0; i < 400; i++ {
		gear := g.genGear(1+i%25, GearSpec{Quality: -1, Rarity: -1, RarityBoost: 1})
		names[gear.Name] = true
		rarities[gear.Rarity] = true
		if gear.Name == "" || len(gear.Lines) < 2 || len(gear.Stats) == 0 {
			t.Fatalf("broken gear %+v", gear)
		}
	}
	if len(names) < 300 || len(rarities) < 5 {
		t.Fatalf("names=%d rarities=%d", len(names), len(rarities))
	}
	p.Ch.Inventory = nil
	before := g.computeStats(p.Ch).Atk
	sword := g.genGear(10, GearSpec{Base: "longsword", Mat: "steel", Quality: 2, Rarity: 2})
	p.Ch.inv(g.pack).AddGear(sword)
	g.useItem(p, 0)
	if p.Ch.Equip["weapon"] == nil || p.Ch.Equip["weapon"].G != sword {
		t.Fatal("not equipped")
	}
	if g.computeStats(p.Ch).Atk <= before {
		t.Fatal("weapon did not raise attack")
	}
	g.unequip(p, "weapon")
	if p.Ch.Equip["weapon"] != nil || len(p.Ch.Inventory) != 1 {
		t.Fatal("unequip failed")
	}
	// Gear survives a save round trip with its stats.
	data, _ := json.Marshal(p.Ch)
	var back Character
	if err := json.Unmarshal(data, &back); err != nil || back.Inventory[0].G == nil || back.Inventory[0].G.Name != sword.Name {
		t.Fatal("gear lost in save")
	}
}

func TestSpellsCostManaAndHurt(t *testing.T) {
	g, p := newTestGame(t)
	var id int64
	var e *Enemy
	for eid, en := range g.enemies {
		if en.Dungeon < 0 && en.Elem.ID != "fire" {
			id, e = eid, en
			break
		}
	}
	e.MaxHP, e.HP = 10000, 10000
	p.Ch.X, p.Ch.Y = e.X+3, e.Y
	p.Target = id
	mana := p.Ch.Mana
	g.input(p, protocol.ClientMsg{T: "cast", Spell: 0}) // firebolt
	if e.HP >= 10000 {
		t.Fatal("firebolt did no damage")
	}
	if p.Ch.Mana >= mana {
		t.Fatal("no mana spent")
	}
	hp := e.HP
	g.input(p, protocol.ClientMsg{T: "cast", Spell: 0})
	if e.HP != hp {
		t.Fatal("cooldown ignored")
	}
	// Learning from a tome binds the new spell.
	p.Ch.Level = 5
	p.Ch.inv(g.pack).Add("tome_frost_nova", 1)
	for i, s := range p.Ch.Inventory {
		if s.Item == "tome_frost_nova" {
			g.useItem(p, i)
		}
	}
	if !p.Ch.knows("frost_nova") || p.Ch.SpellBar[2] != "frost_nova" {
		t.Fatalf("tome not learned: %v", p.Ch.SpellBar)
	}
}

func TestPerksAndAttributes(t *testing.T) {
	g, p := newTestGame(t)
	g.giveXP(p, XPForNext(1)+XPForNext(2)+XPForNext(3)+XPForNext(4))
	if p.Ch.Level != 5 || p.Ch.StatPoints != 8 || p.Ch.PerkPoints != 4 {
		t.Fatalf("level %d stat %d perk %d", p.Ch.Level, p.Ch.StatPoints, p.Ch.PerkPoints)
	}
	atk := g.computeStats(p.Ch).Atk
	g.menuChoice(p, "char", "attr:0", "")
	g.menuChoice(p, "char", "perk:might", "")
	if p.Ch.Attrs[0] != 1 || !p.Ch.hasPerk("might") || g.computeStats(p.Ch).Atk <= atk {
		t.Fatal("attribute or perk did not apply")
	}
	g.menuChoice(p, "char", "perk:juggernaut", "")
	if p.Ch.hasPerk("juggernaut") {
		t.Fatal("tier 4 perk learned too early")
	}
}

func TestTavernCourtesanDiseaseAndHealer(t *testing.T) {
	g, p := newTestGame(t)
	var rosa *NPC
	for _, n := range g.npcs {
		if n.Name == "Rosa" {
			rosa = n
		}
	}
	if rosa == nil {
		t.Fatal("no Rosa in the Drunken Griffin")
	}
	p.Ch.Gold = 10000
	p.Ch.Mood = 10
	diseased := false
	for i := 0; i < 40 && !diseased; i++ {
		standAt(p, rosa)
		g.talk(p, protocol.ClientMsg{T: "talk", NPC: rosa.ID, Opt: "night"})
		for _, s := range p.Ch.Statuses {
			if d := g.pack.Status(s.ID); d != nil && d.Disease {
				diseased = true
			}
		}
	}
	if !diseased {
		t.Fatal("40 cheap nights and not a single disease?")
	}
	if p.Ch.Mood < 30 {
		t.Fatalf("mood = %v", p.Ch.Mood)
	}
	p.S = g.computeStats(p.Ch)
	healer := npcOfRole(g, "healer")
	standAt(p, healer)
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: healer.ID, Opt: "cure"})
	for _, s := range p.Ch.Statuses {
		if d := g.pack.Status(s.ID); d != nil && d.Disease {
			t.Fatal("healer did not cure")
		}
	}
	// The expensive lady never makes you sick.
	for _, n := range g.npcs {
		if n.Name == "Lady Vivienne" {
			for i := 0; i < 20; i++ {
				standAt(p, n)
				g.talk(p, protocol.ClientMsg{T: "talk", NPC: n.ID, Opt: "night"})
			}
		}
	}
	for _, s := range p.Ch.Statuses {
		if d := g.pack.Status(s.ID); d != nil && d.Disease {
			t.Fatal("Lady Vivienne is supposed to be safe")
		}
	}
}

func TestContractsHuntAndCull(t *testing.T) {
	g, p := newTestGame(t)
	board := npcOfRole(g, "board")
	standAt(p, board)
	drain(p)
	g.talkTo(p, board)
	d := lastDialog(t, p)
	o, ok := hasOption(d, "accept:")
	if !ok {
		t.Fatal("empty board")
	}
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: board.ID, Opt: o.ID})
	if len(p.Ch.Contracts) != 1 {
		t.Fatal("contract not accepted")
	}
	c := &p.Ch.Contracts[0]
	if c.Kind == "hunt" {
		var target *Enemy
		var id int64
		for eid, e := range g.enemies {
			if e.ContractID == c.ID {
				target, id = e, eid
			}
		}
		if target == nil {
			t.Fatal("hunt target not spawned")
		}
		g.killEnemy(p, id, target)
	} else {
		fam := g.pack.Family(c.Family)
		for i := 0; i < c.Need; i++ {
			e := g.makeEnemy(fam, g.pack.Element("none"), 1, 1, nil, 1, 1)
			e.Zone = c.Zone
			g.enemies[g.newID()] = e
			for eid, en := range g.enemies {
				if en == e {
					g.killEnemy(p, eid, e)
				}
			}
		}
	}
	if !c.Done {
		t.Fatalf("contract not complete: %+v", c)
	}
	gold := p.Ch.Gold
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: board.ID, Opt: "turnin:" + itoa(c.ID)})
	if p.Ch.Gold <= gold || len(p.Ch.Contracts) != 0 {
		t.Fatal("no reward")
	}
	if g.rep(p, board.Faction) <= 0 {
		t.Fatal("no reputation gained")
	}
}

func itoa(v int64) string { b, _ := json.Marshal(v); return string(b) }

func TestForgeEngraveAndEnchant(t *testing.T) {
	g, p := newTestGame(t)
	smith := npcOfRole(g, "blacksmith")
	standAt(p, smith)
	p.Ch.Inventory = nil
	p.Ch.Gold = 5000
	p.Ch.inv(g.pack).Add("iron_ore", 10)
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: smith.ID, Opt: "forge:sword:iron:2"})
	if len(p.Ch.Inventory) != 2 || p.Ch.Inventory[1].G == nil {
		t.Fatalf("forge failed: %+v", p.Ch.Inventory)
	}
	gear := p.Ch.Inventory[1].G
	if gear.Crafter != "t" || gear.Base != "sword" || gear.Mat != "iron" {
		t.Fatalf("wrong item %+v", gear)
	}
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: smith.ID, Opt: "submit", Menu: "engrave:1", Text: "Toothpick"})
	if !strings.Contains(gear.Name, "Toothpick") {
		t.Fatalf("engraving failed: %s", gear.Name)
	}
	ench := npcOfRole(g, "enchanter")
	standAt(p, ench)
	p.Ch.inv(g.pack).Add("fire_essence", 3)
	gear.Level = 10
	g.talk(p, protocol.ClientMsg{T: "talk", NPC: ench.ID, Opt: "enchant:i.1:flametongue"})
	if gear.Enchant != "flametongue" || gear.Stats["fire"] <= 0 {
		t.Fatalf("enchant failed: %+v", gear)
	}
}

func TestPetsTameSummonAndFight(t *testing.T) {
	g, p := newTestGame(t)
	var wolf *Enemy
	var id int64
	for eid, e := range g.enemies {
		if e.Fam.ID == "wolf" && e.Dungeon < 0 {
			wolf, id = e, eid
			break
		}
	}
	wolf.Level, wolf.HP = 1, 1
	p.Ch.X, p.Ch.Y = wolf.X+1, wolf.Y
	p.Target = id
	p.Ch.inv(g.pack).Add("tame_rope", 10)
	for i := 0; i < 10 && len(p.Ch.Pets) == 0; i++ {
		wolf.HP = 1
		g.input(p, protocol.ClientMsg{T: "tame"})
	}
	if len(p.Ch.Pets) != 1 || p.Pet == nil {
		t.Fatal("taming failed")
	}
	// The Vintokryl hatches from its egg and flies.
	p.Ch.inv(g.pack).Add("pet_vintokryl", 1)
	for i, s := range p.Ch.Inventory {
		if s.Item == "pet_vintokryl" {
			g.useItem(p, i)
		}
	}
	if len(p.Ch.Pets) != 2 || p.Pet == nil || g.petData(p.Pet).Kind != "vintokryl" || !g.petData(p.Pet).Fly {
		t.Fatal("vintokryl did not hatch")
	}
	// It attacks what attacks its owner.
	var foe *Enemy
	for _, e := range g.enemies {
		if e.Dungeon < 0 && e.ContractID == 0 {
			foe = e
			break
		}
	}
	foe.MaxHP, foe.HP, foe.Dmg = 5000, 5000, 0.01
	p.Ch.X, p.Ch.Y = foe.X+1, foe.Y
	p.Pet.X, p.Pet.Y = foe.X+0.5, foe.Y
	foe.Target = p.ID
	for i := 0; i < 100; i++ {
		g.tickPets()
	}
	if foe.HP >= 5000 {
		t.Fatal("pet did not fight")
	}
}

func TestFactionsRipple(t *testing.T) {
	g, p := newTestGame(t)
	g.changeRep(p, "crown", 1000)
	if g.rep(p, "crown") != 1000 || g.rep(p, "merchants") <= 0 || g.rep(p, "shadows") >= 0 {
		t.Fatalf("crown %d merchants %d shadows %d", g.rep(p, "crown"), g.rep(p, "merchants"), g.rep(p, "shadows"))
	}
	if g.repRank(p, "redcaps").Name != "Hostile" {
		t.Fatal("bandits should start hostile")
	}
	g.changeRep(p, "shadows", -10000)
	exotic := npcOfRole(g, "exotic")
	standAt(p, exotic)
	drain(p)
	g.talkTo(p, exotic)
	if d := lastDialog(t, p); len(d.Options) != 1 {
		t.Fatal("hated customers should be turned away")
	}
}

func TestDungeonEnterLootResetAndPatch(t *testing.T) {
	g, p := newTestGame(t)
	link := g.world.Portals[0]
	p.Ch.X, p.Ch.Y = link.X, link.Y
	g.input(p, protocol.ClientMsg{T: "gather"})
	if p.Ch.X != link.ToX || p.Ch.Y != link.ToY {
		t.Fatal("entry failed")
	}
	d := g.world.Dungeons[0]
	// Loot a cache, then the shared cache is empty.
	for idx, code := range g.world.Terrain {
		x, y := float64(idx%g.world.Size)+.5, float64(idx/g.world.Size)+.5
		if code != world.Chest || !d.Contains(x, y) {
			continue
		}
		p.Ch.X, p.Ch.Y = x, y+1
		p.GatherCD = 0
		before := p.Ch.Gold
		g.interact(p)
		if p.Ch.Gold <= before {
			t.Fatal("cache produced no loot")
		}
		after := p.Ch.Gold
		p.GatherCD = 0
		g.interact(p)
		if p.Ch.Gold != after {
			t.Fatal("cache duplicated loot")
		}
		break
	}
	// Kill the boss, leave, and the dungeon reshuffles with a patch.
	for id, e := range g.enemies {
		if e.Boss && e.Dungeon == 0 {
			g.killEnemy(p, id, e)
		}
	}
	p.Ch.X, p.Ch.Y = g.world.SpawnX, g.world.SpawnY
	layout := g.world.EncodeRect(d.X0, d.Y0, d.Size)
	drain(p)
	for i := 0; i < int((dungeonBossWait+1)*TickRate); i++ {
		g.tickDungeons()
	}
	if g.world.EncodeRect(d.X0, d.Y0, d.Size) == layout {
		t.Fatal("dungeon did not regenerate")
	}
	boss := false
	for _, e := range g.enemies {
		boss = boss || (e.Boss && e.Dungeon == 0)
	}
	if !boss {
		t.Fatal("no new boss")
	}
}

func TestCharacterPersistsAndMigrates(t *testing.T) {
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
	p.Ch.Gold = 1234
	g.changeRep(p, "crown", 500)
	g.adoptPet(p, newPetFromDef(g.pack.PetDef("vintokryl")))
	g.save(p, false)
	data, err := store.Authenticate(p.Name, "test-password")
	if err != nil {
		t.Fatal(err)
	}
	var saved Character
	if err := json.Unmarshal(data, &saved); err != nil {
		t.Fatal(err)
	}
	if saved.Gold != 1234 || saved.Rep["crown"] != 500 || len(saved.Pets) != 1 || len(saved.Spells) == 0 {
		t.Fatalf("not persisted: %+v", saved)
	}
	// An old save with legacy weapon/armor fields migrates into slots.
	var old Character
	json.Unmarshal([]byte(`{"level":4,"hp":50,"gold":10,"weapon":"iron_sword","armor":"iron_armor","inventory":[]}`), &old)
	old.normalize(g.pack)
	if old.Equip["weapon"].Item != "iron_sword" || old.Equip["chest"].Item != "iron_armor" || old.StatPoints != 6 || !old.knows("firebolt") {
		t.Fatalf("migration failed: %+v", old)
	}
}

func TestMenusRespond(t *testing.T) {
	g, p := newTestGame(t)
	for _, m := range []string{"char", "spells", "pets", "rep", "journal", "look"} {
		drain(p)
		g.input(p, protocol.ClientMsg{T: "menu", Menu: m})
		if d := lastDialog(t, p); d.Title == "" || len(d.Options) == 0 {
			t.Fatalf("menu %s is empty", m)
		}
	}
	g.menuChoice(p, "look", "hair", "")
	if p.Ch.Look.Hair != 1 {
		t.Fatal("look did not change")
	}
	// Every NPC answers.
	for _, n := range g.npcs {
		standAt(p, n)
		drain(p)
		g.talkTo(p, n)
		if d := lastDialog(t, p); d.Title == "" {
			t.Fatalf("%s (%s) gave an empty dialog", n.Name, n.Role.ID)
		}
	}
}
