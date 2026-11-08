package main

import (
	"encoding/json"
	"fmt"
	"log"
	"math"
	"math/rand/v2"
	"strings"
	"time"
	"unicode"
)

// Authoritative game simulation. A single goroutine owns all game state and
// runs at a fixed tick rate; connections talk to it only through events.

const (
	tickRate       = 20
	dt             = 1.0 / tickRate
	viewRadius     = 26.0
	playerSpeed    = 5.0
	attackRange    = 1.8
	attackCooldown = 0.45
	pickupRange    = 0.9
	enemyCount     = 320
	enemyRespawn   = 20.0
	aggroRange     = 7.0
	leashRange     = 16.0
	maxInventory   = 20
	saveInterval   = 30.0
)

// Character is the persistent part of a player.
type Character struct {
	Level     int      `json:"level"`
	XP        int      `json:"xp"`
	HP        float64  `json:"hp"`
	Gold      int      `json:"gold"`
	X         float64  `json:"x"`
	Y         float64  `json:"y"`
	Inventory []string `json:"inventory"`
	Weapon    string   `json:"weapon"`
	Armor     string   `json:"armor"`
	Kills     int      `json:"kills"`
}

func NewCharacter() *Character {
	return &Character{Level: 1, HP: 100, Inventory: []string{"potion", "potion", "wood_sword"}}
}

func (c *Character) Clone() *Character {
	cp := *c
	cp.Inventory = append([]string(nil), c.Inventory...)
	return &cp
}

func (c *Character) MaxHP() float64 { return 100 + 20*float64(c.Level-1) }

func (c *Character) Attack() int {
	a := 6 + 3*(c.Level-1)
	if d := Item(c.Weapon); d != nil {
		a += int(d.Value)
	}
	return a
}

func (c *Character) Defense() int {
	a := (c.Level - 1) / 2
	if d := Item(c.Armor); d != nil {
		a += int(d.Value)
	}
	return a
}

func XPForNext(level int) int { return int(40 * math.Pow(float64(level), 1.6)) }

// ClientMsg is any message a logged-in client can send.
type ClientMsg struct {
	T     string  `json:"t"`
	DX    float64 `json:"dx"`
	DY    float64 `json:"dy"`
	Text  string  `json:"text"`
	Index int     `json:"index"`
	Slot  string  `json:"slot"`
}

type Event struct {
	Kind      string // join, input, leave
	ID        int64
	Name      string
	Character *Character
	Out       chan []byte
	Msg       ClientMsg
}

type Player struct {
	ID       int64
	Name     string
	Ch       *Character
	DirX     float64
	DirY     float64
	AttackCD float64
	SinceHit float64
	out      chan []byte
}

func (p *Player) send(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		log.Printf("marshal: %v", err)
		return
	}
	select {
	case p.out <- b:
	default: // client too slow: drop the message rather than stall the world
	}
}

func (p *Player) system(text string) { p.send(chatMsg{"chat", "", text}) }

type Enemy struct {
	Def          *EnemyDef
	Level        int
	HP, MaxHP    float64
	X, Y         float64
	HomeX, HomeY float64
	Target       int64
	AttackCD     float64
	WX, WY       float64
	WanderT      float64
}

type GroundItem struct {
	Item   string
	Amount int
	X, Y   float64
	TTL    float64
}

type fx struct {
	Kind string  `json:"kind"`
	X    float64 `json:"x"`
	Y    float64 `json:"y"`
	Text string  `json:"text"`
}

type chatMsg struct {
	T    string `json:"t"`
	From string `json:"from"`
	Text string `json:"text"`
}

type Game struct {
	world    *World
	mapEnc   string
	catalog  map[string]CatalogEntry
	store    *Store
	players  map[int64]*Player
	enemies  map[int64]*Enemy
	items    map[int64]*GroundItem
	respawns []float64
	fx       []fx
	nextID   int64
	saveT    float64
	events   chan Event
}

func NewGame(seed uint32, store *Store) *Game {
	w := GenerateWorld(seed)
	g := &Game{
		world:   w,
		mapEnc:  w.Encode(),
		catalog: Catalog(),
		store:   store,
		players: map[int64]*Player{},
		enemies: map[int64]*Enemy{},
		items:   map[int64]*GroundItem{},
		saveT:   saveInterval,
		events:  make(chan Event, 1024),
	}
	for range enemyCount {
		g.spawnEnemy()
	}
	return g
}

func dist(ax, ay, bx, by float64) float64 { return math.Hypot(ax-bx, ay-by) }

func randRange(lo, hi float64) float64 { return lo + rand.Float64()*(hi-lo) }

func (g *Game) id() int64 { g.nextID++; return g.nextID }

// Run processes events and ticks until stop is closed, then saves everyone.
func (g *Game) Run(stop <-chan struct{}, done chan<- struct{}) {
	ticker := time.NewTicker(time.Second / tickRate)
	defer ticker.Stop()
	for {
		select {
		case <-stop:
			log.Printf("saving %d player(s)...", len(g.players))
			for _, p := range g.players {
				g.save(p, false)
			}
			close(done)
			return
		case ev := <-g.events:
			g.handle(ev)
		case <-ticker.C:
			g.tick()
		}
	}
}

func (g *Game) save(p *Player, async bool) {
	name, ch := p.Name, p.Ch.Clone()
	f := func() {
		if err := g.store.SaveCharacter(name, ch); err != nil {
			log.Printf("save %s: %v", name, err)
		}
	}
	if async {
		go f()
	} else {
		f()
	}
}

func (g *Game) broadcast(v any) {
	for _, p := range g.players {
		p.send(v)
	}
}

func (g *Game) broadcastSystem(text string) { g.broadcast(chatMsg{"chat", "", text}) }

func (g *Game) addFx(kind string, x, y float64, text string) {
	g.fx = append(g.fx, fx{kind, x, y, text})
}

// ------------------------------------------------------------------ events

func (g *Game) handle(ev Event) {
	switch ev.Kind {
	case "join":
		g.join(ev)
	case "leave":
		p, ok := g.players[ev.ID]
		if !ok {
			return
		}
		delete(g.players, ev.ID)
		close(p.out)
		log.Printf("%s left", p.Name)
		g.save(p, true)
		g.broadcastSystem(p.Name + " left the world")
		for _, e := range g.enemies {
			if e.Target == ev.ID {
				e.Target = 0
			}
		}
	case "input":
		g.input(ev.ID, ev.Msg)
	}
}

func (g *Game) join(ev Event) {
	for _, p := range g.players {
		if p.Name == ev.Name {
			b, _ := json.Marshal(map[string]string{"t": "error", "message": "This character is already online"})
			ev.Out <- b
			close(ev.Out)
			return
		}
	}
	ch := ev.Character
	if ch == nil {
		ch = NewCharacter()
	}
	ch.Level = max(ch.Level, 1)
	if !g.world.Free(ch.X, ch.Y) {
		ch.X, ch.Y = g.world.SpawnX, g.world.SpawnY
	}
	if ch.HP <= 0 {
		ch.HP = ch.MaxHP()
	}
	p := &Player{ID: ev.ID, Name: ev.Name, Ch: ch, SinceHit: 99, out: ev.Out}
	p.send(map[string]any{
		"t": "welcome", "id": p.ID, "name": p.Name,
		"w": MapW, "h": MapH, "map": g.mapEnc, "items": g.catalog,
	})
	p.system(fmt.Sprintf("Welcome to Wildforge, %s! WASD to move, Space to attack, Q to drink a potion, Enter to chat.", p.Name))
	log.Printf("%s joined", p.Name)
	g.players[p.ID] = p
	g.broadcastSystem(p.Name + " entered the world")
}

func (g *Game) input(id int64, m ClientMsg) {
	p, ok := g.players[id]
	if !ok {
		return
	}
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
	case "chat":
		g.chat(p, m.Text)
	case "use":
		g.useItem(p, m.Index)
	case "potion":
		for i, it := range p.Ch.Inventory {
			if d := Item(it); d != nil && d.Slot == SlotPotion {
				g.useItem(p, i)
				return
			}
		}
		p.system("You have no potions.")
	case "drop":
		if m.Index < 0 || m.Index >= len(p.Ch.Inventory) {
			return
		}
		it := p.Ch.Inventory[m.Index]
		p.Ch.Inventory = append(p.Ch.Inventory[:m.Index], p.Ch.Inventory[m.Index+1:]...)
		// Put it a little away so it isn't picked up again immediately.
		g.items[g.id()] = &GroundItem{Item: it, Amount: 1, X: p.Ch.X + 1.2, Y: p.Ch.Y, TTL: 120}
	case "unequip":
		if len(p.Ch.Inventory) >= maxInventory {
			p.system("Inventory full.")
			return
		}
		var slot *string
		switch m.Slot {
		case "weapon":
			slot = &p.Ch.Weapon
		case "armor":
			slot = &p.Ch.Armor
		default:
			return
		}
		if *slot != "" {
			p.Ch.Inventory = append(p.Ch.Inventory, *slot)
			*slot = ""
		}
	}
}

func (g *Game) chat(p *Player, text string) {
	text = strings.Map(func(r rune) rune {
		if unicode.IsControl(r) {
			return -1
		}
		return r
	}, strings.TrimSpace(text))
	if r := []rune(text); len(r) > 200 {
		text = string(r[:200])
	}
	switch text {
	case "":
		return
	case "/who":
		names := make([]string, 0, len(g.players))
		for _, o := range g.players {
			names = append(names, fmt.Sprintf("%s (lv %d)", o.Name, o.Ch.Level))
		}
		p.system(fmt.Sprintf("Online (%d): %s", len(names), strings.Join(names, ", ")))
		return
	case "/stats":
		c := p.Ch
		p.system(fmt.Sprintf("Level %d | XP %d/%d | ATK %d | DEF %d | Gold %d | Kills %d",
			c.Level, c.XP, XPForNext(c.Level), c.Attack(), c.Defense(), c.Gold, c.Kills))
		return
	}
	g.broadcast(chatMsg{"chat", p.Name, text})
}

func (g *Game) useItem(p *Player, index int) {
	if index < 0 || index >= len(p.Ch.Inventory) {
		return
	}
	id := p.Ch.Inventory[index]
	d := Item(id)
	if d == nil {
		return
	}
	remove := func() { p.Ch.Inventory = append(p.Ch.Inventory[:index], p.Ch.Inventory[index+1:]...) }
	switch d.Slot {
	case SlotPotion:
		mx := p.Ch.MaxHP()
		if p.Ch.HP >= mx {
			p.system("You are already at full health.")
			return
		}
		heal := math.Round(mx * d.Value)
		p.Ch.HP = math.Min(p.Ch.HP+heal, mx)
		remove()
		g.addFx("heal", p.Ch.X, p.Ch.Y, fmt.Sprintf("+%d", int(heal)))
	case SlotWeapon, SlotArmor:
		slot := &p.Ch.Weapon
		if d.Slot == SlotArmor {
			slot = &p.Ch.Armor
		}
		remove()
		if *slot != "" {
			p.Ch.Inventory = append(p.Ch.Inventory, *slot)
		}
		*slot = id
		p.system("Equipped " + d.Name + ".")
	}
}

func (g *Game) playerAttack(p *Player) {
	if p.AttackCD > 0 {
		return
	}
	p.AttackCD = attackCooldown
	g.addFx("swing", p.Ch.X, p.Ch.Y, "")

	var targetID int64
	best := attackRange
	for id, e := range g.enemies {
		if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y); d <= best {
			best, targetID = d, id
		}
	}
	e, ok := g.enemies[targetID]
	if !ok {
		return
	}
	dmg := math.Max(1, math.Round(float64(p.Ch.Attack())*randRange(0.8, 1.2)))
	text := fmt.Sprint(dmg)
	if rand.Float64() < 0.1 {
		dmg *= 2
		text = fmt.Sprintf("%v!", dmg)
	}
	e.HP -= dmg
	e.Target = p.ID
	g.addFx("hit", e.X, e.Y, text)
	if e.HP <= 0 {
		delete(g.enemies, targetID)
		g.respawns = append(g.respawns, enemyRespawn)
		g.onEnemyKilled(p, e)
	}
}

func (g *Game) onEnemyKilled(p *Player, e *Enemy) {
	xp := e.Def.XP * e.Level
	c := p.Ch
	c.Kills++
	c.XP += xp
	g.addFx("xp", c.X, c.Y-0.6, fmt.Sprintf("+%d XP", xp))
	leveled := false
	for c.XP >= XPForNext(c.Level) {
		c.XP -= XPForNext(c.Level)
		c.Level++
		leveled = true
	}
	if leveled {
		c.HP = c.MaxHP()
		g.addFx("level", c.X, c.Y, fmt.Sprintf("LEVEL %d!", c.Level))
		g.broadcastSystem(fmt.Sprintf("%s reached level %d!", p.Name, c.Level))
	}
	// Loot
	g.dropItem("gold", e.Level*(1+rand.IntN(4)), e.X, e.Y)
	if rand.Float64() < 0.3 {
		potion := "potion"
		if e.Level >= 6 && rand.Float64() < 0.4 {
			potion = "big_potion"
		}
		g.dropItem(potion, 1, e.X, e.Y)
	}
	if rand.Float64() < 0.1 {
		tier := min(e.Level/4+rand.IntN(2), 3)
		it := ArmorTiers[tier]
		if rand.Float64() < 0.5 {
			it = WeaponTiers[tier]
		}
		g.dropItem(it, 1, e.X, e.Y)
	}
}

func (g *Game) dropItem(item string, amount int, x, y float64) {
	g.items[g.id()] = &GroundItem{
		Item: item, Amount: amount, TTL: 90,
		X: x + randRange(-0.5, 0.5), Y: y + randRange(-0.5, 0.5),
	}
}

func (g *Game) spawnEnemy() {
	sx, sy := g.world.SpawnX, g.world.SpawnY
outer:
	for range 200 {
		x, y := randRange(2, MapW-2), randRange(2, MapH-2)
		d := dist(x, y, sx, sy)
		if d < 12 || !g.world.Free(x, y) {
			continue
		}
		for _, p := range g.players {
			if dist(p.Ch.X, p.Ch.Y, x, y) < 12 {
				continue outer
			}
		}
		level := 1 + int(d/9) + rand.IntN(2)
		var cands []*EnemyDef
		for i := range Enemies {
			if Enemies[i].MinLevel <= level {
				cands = append(cands, &Enemies[i])
			}
		}
		def := cands[rand.IntN(len(cands))]
		hp := def.BaseHP + def.HPPerLevel*float64(level-1)
		g.enemies[g.id()] = &Enemy{Def: def, Level: level, HP: hp, MaxHP: hp, X: x, Y: y, HomeX: x, HomeY: y}
		return
	}
}

// -------------------------------------------------------------------- tick

func (g *Game) tick() {
	// Players: movement, regen, pickup.
	for _, p := range g.players {
		c := p.Ch
		if p.DirX != 0 || p.DirY != 0 {
			c.X, c.Y = g.world.Step(c.X, c.Y, p.DirX*playerSpeed*dt, p.DirY*playerSpeed*dt)
		}
		p.AttackCD -= dt
		p.SinceHit += dt
		if p.SinceHit > 5 {
			c.HP = math.Min(c.HP+c.MaxHP()*0.03*dt, c.MaxHP())
		}
		for iid, it := range g.items {
			if dist(c.X, c.Y, it.X, it.Y) >= pickupRange {
				continue
			}
			if it.Item == "gold" {
				c.Gold += it.Amount
				g.addFx("gold", it.X, it.Y, fmt.Sprintf("+%dg", it.Amount))
			} else if len(c.Inventory) < maxInventory {
				c.Inventory = append(c.Inventory, it.Item)
				name := it.Item
				if d := Item(it.Item); d != nil {
					name = d.Name
				}
				g.addFx("loot", it.X, it.Y, name)
			} else {
				continue
			}
			delete(g.items, iid)
		}
	}

	// Enemies: simple chase / wander AI.
	for _, e := range g.enemies {
		e.AttackCD -= dt
		if e.Target == 0 {
			best := aggroRange
			for id, p := range g.players {
				if d := dist(p.Ch.X, p.Ch.Y, e.X, e.Y); d < best {
					best, e.Target = d, id
				}
			}
		}
		t, chasing := g.players[e.Target]
		if chasing && dist(e.X, e.Y, e.HomeX, e.HomeY) < leashRange {
			d := dist(e.X, e.Y, t.Ch.X, t.Ch.Y)
			if d > 0.9 {
				s := e.Def.Speed * dt / d
				e.X, e.Y = g.world.Step(e.X, e.Y, (t.Ch.X-e.X)*s, (t.Ch.Y-e.Y)*s)
			} else if e.AttackCD <= 0 {
				e.AttackCD = 1
				g.enemyHit(t, e.Def.BaseDmg+e.Def.DmgPerLevel*float64(e.Level-1))
			}
			if d > aggroRange*2 {
				e.Target = 0
			}
			continue
		}
		// Wander, drift home if too far, heal while idle.
		e.Target = 0
		e.HP = math.Min(e.HP+e.MaxHP*0.05*dt, e.MaxHP)
		e.WanderT -= dt
		if hd := dist(e.X, e.Y, e.HomeX, e.HomeY); hd > 6 {
			e.WX, e.WY = (e.HomeX-e.X)/hd, (e.HomeY-e.Y)/hd
		} else if e.WanderT <= 0 {
			e.WanderT = randRange(1, 4)
			if rand.Float64() < 0.4 {
				e.WX, e.WY = 0, 0
			} else {
				a := rand.Float64() * 2 * math.Pi
				e.WX, e.WY = math.Cos(a), math.Sin(a)
			}
		}
		s := e.Def.Speed * 0.4 * dt
		e.X, e.Y = g.world.Step(e.X, e.Y, e.WX*s, e.WY*s)
	}

	// Ground items expire.
	for id, it := range g.items {
		if it.TTL -= dt; it.TTL <= 0 {
			delete(g.items, id)
		}
	}

	// Enemy respawns.
	ready := 0
	kept := g.respawns[:0]
	for _, t := range g.respawns {
		if t -= dt; t <= 0 {
			ready++
		} else {
			kept = append(kept, t)
		}
	}
	g.respawns = kept
	for range ready {
		g.spawnEnemy()
	}

	// Periodic save.
	if g.saveT -= dt; g.saveT <= 0 {
		g.saveT = saveInterval
		for _, p := range g.players {
			g.save(p, true)
		}
	}

	g.broadcastState()
	g.fx = g.fx[:0]
}

func (g *Game) enemyHit(p *Player, raw float64) {
	c := p.Ch
	dmg := math.Max(1, math.Round(raw*randRange(0.8, 1.2)-float64(c.Defense())))
	c.HP -= dmg
	p.SinceHit = 0
	g.addFx("hurt", c.X, c.Y, fmt.Sprintf("-%d", int(dmg)))
	if c.HP > 0 {
		return
	}
	lost := c.Gold / 10
	c.Gold -= lost
	c.HP = c.MaxHP()
	c.X, c.Y = g.world.SpawnX, g.world.SpawnY
	p.DirX, p.DirY = 0, 0
	p.send(map[string]any{"t": "died", "lost_gold": lost})
	for _, e := range g.enemies {
		if e.Target == p.ID {
			e.Target = 0
		}
	}
	g.broadcastSystem(p.Name + " has fallen")
}

type youState struct {
	ID        int64    `json:"id"`
	X         float64  `json:"x"`
	Y         float64  `json:"y"`
	HP        float64  `json:"hp"`
	MaxHP     float64  `json:"max_hp"`
	Level     int      `json:"level"`
	XP        int      `json:"xp"`
	XPNext    int      `json:"xp_next"`
	Gold      int      `json:"gold"`
	Attack    int      `json:"attack"`
	Defense   int      `json:"defense"`
	Inventory []string `json:"inventory"`
	Weapon    string   `json:"weapon"`
	Armor     string   `json:"armor"`
	Kills     int      `json:"kills"`
	Online    int      `json:"online"`
}

type playerState struct {
	ID     int64   `json:"id"`
	Name   string  `json:"name"`
	X      float64 `json:"x"`
	Y      float64 `json:"y"`
	HP     float64 `json:"hp"`
	MaxHP  float64 `json:"max_hp"`
	Level  int     `json:"level"`
	Weapon string  `json:"weapon"`
}

type enemyState struct {
	ID    int64   `json:"id"`
	Kind  string  `json:"kind"`
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	HP    float64 `json:"hp"`
	MaxHP float64 `json:"max_hp"`
	Level int     `json:"level"`
}

type itemState struct {
	ID   int64   `json:"id"`
	Item string  `json:"item"`
	X    float64 `json:"x"`
	Y    float64 `json:"y"`
}

type stateMsg struct {
	T       string        `json:"t"`
	You     youState      `json:"you"`
	Players []playerState `json:"players"`
	Enemies []enemyState  `json:"enemies"`
	Items   []itemState   `json:"items"`
	Fx      []fx          `json:"fx"`
}

func (g *Game) broadcastState() {
	for _, p := range g.players {
		c := p.Ch
		near := func(x, y float64) bool {
			return math.Abs(x-c.X) < viewRadius && math.Abs(y-c.Y) < viewRadius
		}
		m := stateMsg{T: "state", Players: []playerState{}, Enemies: []enemyState{}, Items: []itemState{}, Fx: []fx{}}
		m.You = youState{
			ID: p.ID, X: c.X, Y: c.Y, HP: math.Round(c.HP), MaxHP: c.MaxHP(), Level: c.Level,
			XP: c.XP, XPNext: XPForNext(c.Level), Gold: c.Gold, Attack: c.Attack(), Defense: c.Defense(),
			Inventory: c.Inventory, Weapon: c.Weapon, Armor: c.Armor, Kills: c.Kills, Online: len(g.players),
		}
		for _, o := range g.players {
			if near(o.Ch.X, o.Ch.Y) {
				m.Players = append(m.Players, playerState{o.ID, o.Name, o.Ch.X, o.Ch.Y, math.Round(o.Ch.HP), o.Ch.MaxHP(), o.Ch.Level, o.Ch.Weapon})
			}
		}
		for id, e := range g.enemies {
			if near(e.X, e.Y) {
				m.Enemies = append(m.Enemies, enemyState{id, e.Def.Kind, e.X, e.Y, math.Round(e.HP), e.MaxHP, e.Level})
			}
		}
		for id, it := range g.items {
			if near(it.X, it.Y) {
				m.Items = append(m.Items, itemState{id, it.Item, it.X, it.Y})
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
