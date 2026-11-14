// Package game is the authoritative simulation. One goroutine owns all game
// state and advances it at a fixed tick rate; network sessions only talk to
// it through the event channel (Join / Input / Leave).
package game

import (
	"encoding/json"
	"log"
	"math"
	"math/rand/v2"
	"time"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/world"
)

const (
	TickRate        = 20
	dt              = 1.0 / TickRate
	viewRadius      = 34.0
	playerSpeed     = 5.0
	attackRange     = 1.9
	attackCooldown  = 0.45
	gatherRange     = 1.7
	gatherCooldown  = 0.5
	pickupRange     = 0.9
	aggroRange      = 7.0
	leashRange      = 16.0
	saveInterval    = 30.0
	dayLength       = 900.0 // seconds per in-game day
	contractRefresh = 600.0
)

// Saver persists a character; implemented by storage.Store.
type Saver interface {
	SaveCharacter(name string, character json.RawMessage) error
}

type eventKind int

const (
	evJoin eventKind = iota
	evInput
	evLeave
)

type event struct {
	kind      eventKind
	id        int64
	name      string
	character json.RawMessage
	out       chan []byte
	msg       protocol.ClientMsg
}

type GroundItem struct {
	content.Stack
	X, Y float64
	TTL  float64
}

type Game struct {
	pack  *content.Pack
	world *world.World
	saver Saver

	terrainEnc string
	nodesEnc   string
	zonesEnc   string

	players     map[int64]*Player
	enemies     map[int64]*Enemy
	npcs        map[int64]*NPC
	pets        map[int64]*PetEnt
	items       map[int64]*GroundItem
	enemyTimers []float64
	nodeTimers  map[int]float64
	chestTimers map[int]float64
	dungeons    map[int]*dungeonState
	boards      [][]Contract
	fx          []protocol.Fx
	nextID      int64
	saveT       float64
	clock       float64
	contractT   float64
	events      chan event
}

func New(pack *content.Pack, w *world.World, saver Saver) *Game {
	g := &Game{
		pack: pack, world: w, saver: saver,
		terrainEnc: w.EncodeTerrain(), nodesEnc: w.EncodeNodes(), zonesEnc: w.EncodeZones(),
		players: map[int64]*Player{}, enemies: map[int64]*Enemy{}, npcs: map[int64]*NPC{}, pets: map[int64]*PetEnt{},
		items: map[int64]*GroundItem{}, nodeTimers: map[int]float64{}, chestTimers: map[int]float64{},
		dungeons: map[int]*dungeonState{}, saveT: saveInterval, contractT: contractRefresh, events: make(chan event, 1024),
		clock: dayLength * 0.1,
	}
	for range pack.World.EnemyCount {
		g.spawnEnemy()
	}
	for i := range w.Dungeons {
		g.spawnDungeon(i)
	}
	g.spawnNPCs()
	g.refreshContracts()
	return g
}

// ---- API used by network sessions (safe from any goroutine)

// Join adds a logged-in player. Messages for the client are sent on out; the
// game closes out when the player is removed or rejected.
func (g *Game) Join(id int64, name string, character json.RawMessage, out chan []byte) {
	g.events <- event{kind: evJoin, id: id, name: name, character: character, out: out}
}

func (g *Game) Input(id int64, msg protocol.ClientMsg) {
	g.events <- event{kind: evInput, id: id, msg: msg}
}

func (g *Game) Leave(id int64) { g.events <- event{kind: evLeave, id: id} }

// Run processes events and ticks until stop is closed, then saves everyone.
func (g *Game) Run(stop <-chan struct{}) {
	ticker := time.NewTicker(time.Second / TickRate)
	defer ticker.Stop()
	for {
		select {
		case <-stop:
			log.Printf("saving %d player(s)...", len(g.players))
			for _, p := range g.players {
				g.save(p, false)
			}
			return
		case ev := <-g.events:
			switch ev.kind {
			case evJoin:
				g.join(ev)
			case evLeave:
				g.leave(ev.id)
			case evInput:
				if p, ok := g.players[ev.id]; ok {
					g.input(p, ev.msg)
				}
			}
		case <-ticker.C:
			g.tick()
		}
	}
}

// ---- helpers shared by the systems

func (g *Game) newID() int64 { g.nextID++; return g.nextID }

func dist(ax, ay, bx, by float64) float64 { return math.Hypot(ax-bx, ay-by) }

func randRange(lo, hi float64) float64 { return lo + rand.Float64()*(hi-lo) }

func randInt(lo, hi int) int { return lo + rand.IntN(max(1, hi-lo+1)) }

func (g *Game) broadcast(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		log.Printf("marshal: %v", err)
		return
	}
	for _, p := range g.players {
		p.sendRaw(b)
	}
}

func (g *Game) broadcastSystem(text string) { g.broadcast(protocol.System(text)) }

func (g *Game) addFx(kind string, x, y float64, text string) {
	g.fx = append(g.fx, protocol.Fx{Kind: kind, X: x, Y: y, Text: text})
}

func (g *Game) save(p *Player, async bool) {
	name := p.Name
	data, err := json.Marshal(p.Ch.clone())
	if err != nil {
		log.Printf("save %s: %v", name, err)
		return
	}
	f := func() {
		if err := g.saver.SaveCharacter(name, data); err != nil {
			log.Printf("save %s: %v", name, err)
		}
	}
	if async {
		go f()
	} else {
		f()
	}
}

// TimeOfDay is 0..1 (0 = midnight, 0.5 = noon).
func (g *Game) TimeOfDay() float64 { return math.Mod(g.clock/dayLength+0.3, 1) }

// ---- tick

func (g *Game) tick() {
	g.clock += dt
	for i, t := range g.chestTimers {
		if t <= dt {
			delete(g.chestTimers, i)
		} else {
			g.chestTimers[i] = t - dt
		}
	}
	g.tickPlayers()
	g.tickEnemies()
	g.tickPets()
	g.tickGroundItems()
	g.tickNodes()
	g.tickRespawns()
	g.tickDungeons()
	if g.contractT -= dt; g.contractT <= 0 {
		g.contractT = contractRefresh
		g.refreshContracts()
	}
	if g.saveT -= dt; g.saveT <= 0 {
		g.saveT = saveInterval
		for _, p := range g.players {
			g.save(p, true)
		}
	}
	g.broadcastState()
	g.fx = g.fx[:0]
}

func (g *Game) tickRespawns() {
	ready := 0
	kept := g.enemyTimers[:0]
	for _, t := range g.enemyTimers {
		if t -= dt; t <= 0 {
			ready++
		} else {
			kept = append(kept, t)
		}
	}
	g.enemyTimers = kept
	for range ready {
		g.spawnEnemy()
	}
}
