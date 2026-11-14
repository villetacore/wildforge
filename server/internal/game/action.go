package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

// Action combat in the spirit of souls-likes: stamina-gated light and heavy
// attacks that chain into combos and hit everything in a cone in front of the
// player after a wind-up, dodge rolls with invulnerability frames, jumps,
// blocking with a timed parry, and sprinting.

const (
	staminaBase   = 100.0
	staminaRegen  = 32.0
	staminaDelay  = 0.7
	comboWindow   = 0.55 // after an attack ends, the next one continues the chain
	inputBuffer   = 0.3  // an attack pressed this close to the end of the current one is queued
	dodgeTime     = 0.5
	dodgeIFrames  = 0.38
	dodgeSpeed    = 9.0
	jumpTime      = 0.62
	parryWindow   = 0.25
	riposteWindow = 1.8
	sprintMult    = 1.55
	sprintDrain   = 14.0
	blockMult     = 0.5
	blockDrain    = 0.9 // stamina lost per point of blocked damage
	staminaPerHit = 1.0
)

type attackDef struct {
	anim          string
	cost          float64
	duration, hit float64 // total action time and the moment damage lands
	mult, reach   float64
	cone          float64 // half-angle in radians
	lunge         float64 // tiles moved forward during the swing
	stagger       float64
}

var (
	lightChain = []attackDef{
		{"light1", 9, 0.42, 0.16, 1.0, 2.0, 0.9, 0.5, 0},
		{"light2", 9, 0.42, 0.16, 1.1, 2.0, 0.9, 0.5, 0},
		{"light3", 12, 0.62, 0.3, 1.6, 2.3, 1.6, 0.9, 0.5},
	}
	heavyChain = []attackDef{
		{"heavy1", 20, 0.85, 0.48, 2.2, 2.4, 1.2, 0.7, 1.2},
		{"heavy2", 22, 0.9, 0.5, 2.5, 2.6, 1.4, 1.0, 1.4},
	}
	finisher = attackDef{"finisher", 24, 1.0, 0.55, 3.2, 2.8, 2.0, 1.2, 2.0}
)

// Action is the player's current combat state.
type Action struct {
	Stamina   float64
	RestT     float64 // time since stamina was last spent
	Anim      string
	Seq       int
	T         float64 // time left in the current action
	Atk       *attackDef
	HitDone   bool
	Elapsed   float64
	Combo     int // light chain position (0 = none)
	HeavyStep int
	ComboT    float64
	Queued    string
	DodgeT    float64
	DodgeX    float64
	DodgeY    float64
	AirT      float64
	Blocking  bool
	BlockT    float64 // time since block was raised
	Sprint    bool
	FaceX     float64
	FaceY     float64
}

func (p *Player) maxStamina() float64 { return staminaBase + p.S.Attr[attrDex]*2 + p.S.Attr[attrVit] }

func (p *Player) anim(name string) {
	p.A.Anim = name
	p.A.Seq++
}

func (p *Player) spend(cost float64) bool {
	if p.A.Stamina <= 0 {
		return false
	}
	p.A.Stamina -= cost
	p.A.RestT = 0
	return true
}

// busy reports whether an action locks out new ones.
func (p *Player) busy() bool { return p.A.T > 0 }

func (g *Game) face(p *Player, dx, dy float64) {
	// A locked target decides where we swing.
	if e, ok := g.enemies[p.Target]; ok && dist(e.X, e.Y, p.Ch.X, p.Ch.Y) < 6 {
		dx, dy = e.X-p.Ch.X, e.Y-p.Ch.Y
	}
	if l := math.Hypot(dx, dy); l > 0.01 && !math.IsNaN(l) {
		p.A.FaceX, p.A.FaceY = dx/l, dy/l
	} else if p.A.FaceX == 0 && p.A.FaceY == 0 {
		p.A.FaceX, p.A.FaceY = 0, 1
	}
}

// attackInput handles a light or heavy attack press facing (dx, dy).
func (g *Game) attackInput(p *Player, kind string, dx, dy float64) {
	if g.mods(p.Ch.Statuses).stun || p.A.DodgeT > 0 {
		return
	}
	if p.busy() {
		if p.A.T <= inputBuffer {
			p.A.Queued = kind
		}
		return
	}
	g.face(p, dx, dy)
	var def attackDef
	switch {
	case kind == "heavy" && p.A.ComboT > 0 && p.A.Combo >= 2:
		def = finisher
		p.A.Combo, p.A.HeavyStep = 0, 0
	case kind == "heavy":
		if p.A.ComboT <= 0 || p.A.HeavyStep >= len(heavyChain) {
			p.A.HeavyStep = 0
		}
		def = heavyChain[p.A.HeavyStep]
		p.A.HeavyStep++
		p.A.Combo = 0
	default:
		if p.A.ComboT <= 0 || p.A.Combo >= len(lightChain) {
			p.A.Combo = 0
		}
		def = lightChain[p.A.Combo]
		p.A.Combo++
		p.A.HeavyStep = 0
	}
	if !p.spend(def.cost) {
		p.system("Too exhausted to attack.")
		return
	}
	p.A.Blocking = false
	p.A.Atk = &def
	p.A.T, p.A.Elapsed, p.A.HitDone = def.duration, 0, false
	p.A.Queued = ""
	p.anim(def.anim)
	g.addFx("swing", p.Ch.X, p.Ch.Y, "")
}

func (g *Game) dodge(p *Player, dx, dy float64) {
	if p.A.DodgeT > 0 || g.mods(p.Ch.Statuses).stun || p.A.Stamina < 10 {
		return
	}
	// Dodging cancels the recovery of an attack, but not its wind-up.
	if p.A.Atk != nil && !p.A.HitDone {
		return
	}
	if l := math.Hypot(dx, dy); l > 0.01 && !math.IsNaN(l) {
		dx, dy = dx/l, dy/l
	} else {
		dx, dy = -p.A.FaceX, -p.A.FaceY // backstep
	}
	p.spend(22)
	p.A.Atk, p.A.Blocking, p.A.Queued = nil, false, ""
	p.A.DodgeT, p.A.DodgeX, p.A.DodgeY = dodgeTime, dx, dy
	p.A.T = dodgeTime
	p.A.FaceX, p.A.FaceY = dx, dy
	p.anim("dodge")
}

func (g *Game) jump(p *Player) {
	if p.A.AirT > 0 || p.busy() || p.A.Stamina < 5 {
		return
	}
	p.spend(8)
	p.A.AirT = jumpTime
	p.anim("jump")
}

func (g *Game) setBlock(p *Player, on bool) {
	if on == p.A.Blocking {
		return
	}
	if on && (p.busy() || p.A.Stamina <= 0) {
		return
	}
	p.A.Blocking = on
	p.A.BlockT = 0
	if on {
		p.anim("block")
	} else {
		p.anim("idle")
	}
}

// tickAction advances timers, moves the player during rolls and lunges, and
// lands attacks at the right moment of their swing.
func (g *Game) tickAction(p *Player) {
	a := &p.A
	c := p.Ch
	a.T = math.Max(0, a.T-dt)
	a.ComboT -= dt
	a.RestT += dt
	a.BlockT += dt
	if a.AirT > 0 {
		a.AirT -= dt
	}
	// Stamina.
	mx := p.maxStamina()
	switch {
	case a.Sprint && (p.DirX != 0 || p.DirY != 0) && !a.Blocking:
		a.Stamina -= sprintDrain * dt
		a.RestT = 0
		if a.Stamina <= 0 {
			a.Sprint = false
		}
	case a.RestT > staminaDelay:
		rate := staminaRegen
		if a.Blocking {
			rate *= 0.35
		}
		a.Stamina = math.Min(mx, a.Stamina+rate*dt)
	}
	a.Stamina = math.Max(-20, math.Min(a.Stamina, mx))
	// Dodge roll movement.
	if a.DodgeT > 0 {
		a.DodgeT -= dt
		if a.DodgeT > dodgeTime-0.36 {
			sp := dodgeSpeed * dt
			c.X, c.Y = g.world.Step(c.X, c.Y, a.DodgeX*sp, a.DodgeY*sp)
		}
		if a.DodgeT <= 0 {
			a.T = 0
		}
	}
	// Attack: lunge, then land the blow, then recover.
	if at := a.Atk; at != nil {
		a.Elapsed += dt
		if a.Elapsed < at.hit && !g.enemyAhead(p, 1.1) {
			sp := at.lunge / at.hit * dt
			c.X, c.Y = g.world.Step(c.X, c.Y, a.FaceX*sp, a.FaceY*sp)
		}
		if !a.HitDone && a.Elapsed >= at.hit {
			a.HitDone = true
			g.landAttack(p, at)
		}
		if a.T <= 0 {
			a.Atk = nil
			a.ComboT = comboWindow
			if q := a.Queued; q != "" {
				a.Queued = ""
				g.attackInput(p, q, a.FaceX, a.FaceY)
			} else {
				p.anim("idle")
			}
		}
	}
}

// enemyAhead stops lunges at the enemy instead of passing through it.
func (g *Game) enemyAhead(p *Player, rng float64) bool {
	for _, e := range g.enemies {
		dx, dy := e.X-p.Ch.X, e.Y-p.Ch.Y
		if d := math.Hypot(dx, dy); d < rng+0.3*e.Scale && dx*p.A.FaceX+dy*p.A.FaceY > 0 {
			return true
		}
	}
	return false
}

// landAttack damages every enemy in the cone in front of the player.
func (g *Game) landAttack(p *Player, at *attackDef) {
	c := p.Ch
	s := &p.S
	hits := 0
	for id, e := range g.enemies {
		dx, dy := e.X-c.X, e.Y-c.Y
		d := math.Hypot(dx, dy) - 0.3*(e.Scale-1)
		if d > at.reach || !g.world.ClearLine(c.X, c.Y, e.X, e.Y) {
			continue
		}
		if d > 0.4 {
			ang := math.Acos(math.Max(-1, math.Min(1, (dx*p.A.FaceX+dy*p.A.FaceY)/math.Hypot(dx, dy))))
			if ang > at.cone {
				continue
			}
		}
		dmg := s.Atk * at.mult * randRange(0.9, 1.1) * g.mods(c.Statuses).dealt
		if s.Opener > 0 && e.HP >= e.MaxHP {
			dmg *= 1 + s.Opener
		}
		crit := rand.Float64()*100 < s.Crit
		if e.StaggerT > 0 && e.Riposte {
			dmg *= 2.5 // riposte after a parry
			crit = true
			e.Riposte = false
			g.addFx("level", e.X, e.Y, "RIPOSTE!")
		} else if crit {
			dmg *= s.CritMult
		}
		if hits == 0 && p.Target == 0 {
			p.Target = id
		}
		hits++
		g.damageEnemy(p, id, e, dmg, "phys", crit, true)
		if _, alive := g.enemies[id]; !alive {
			continue
		}
		for _, el := range elements {
			if v := s.Elem[el]; v > 0 {
				g.damageEnemy(p, id, e, v*randRange(0.8, 1.2)*at.mult, el, false, false)
				if _, alive := g.enemies[id]; !alive {
					break
				}
			}
		}
		if _, alive := g.enemies[id]; alive {
			if at.stagger > 0 {
				g.staggerEnemy(e, at.stagger)
			}
			if s.PoisonHit > 0 && rand.Float64() < s.PoisonHit {
				g.addStatus(&e.Statuses, "poison", 1+float64(c.Level)*0.3)
			}
		}
	}
	if hits > 0 {
		p.A.Stamina = math.Min(p.maxStamina(), p.A.Stamina+staminaPerHit*float64(hits))
		if at.stagger > 0 {
			g.fx = append(g.fx, protocol.Fx{Kind: "shake", X: c.X, Y: c.Y, R: 0.25 * at.mult})
		}
	}
}

func (g *Game) staggerEnemy(e *Enemy, t float64) {
	if e.Boss {
		t *= 0.4
	}
	if t > e.StaggerT {
		e.StaggerT = t
		e.Windup = 0
		e.anim("stagger")
	}
}

// defend applies dodge, jump and block rules to an incoming enemy blow.
// It returns the damage that still lands (0 if avoided).
func (g *Game) defend(p *Player, raw float64, src *Enemy) float64 {
	a := &p.A
	if a.DodgeT > dodgeTime-dodgeIFrames {
		g.addFx("hurt", p.Ch.X, p.Ch.Y, "dodged")
		return 0
	}
	if src == nil {
		return raw
	}
	if a.AirT > 0 && src.Fam.Range == 0 && src.Fam.Model != "bat" && src.Fam.Model != "wisp" {
		g.addFx("hurt", p.Ch.X, p.Ch.Y, "evaded")
		return 0
	}
	if !a.Blocking {
		return raw
	}
	// Blocking only covers the front.
	dx, dy := src.X-p.Ch.X, src.Y-p.Ch.Y
	if l := math.Hypot(dx, dy); l > 0.01 && (dx*a.FaceX+dy*a.FaceY)/l < 0.2 {
		return raw
	}
	if a.BlockT <= parryWindow {
		g.staggerEnemy(src, 1.6)
		src.Riposte = true
		p.anim("parry")
		g.addFx("level", p.Ch.X, p.Ch.Y, "PARRY!")
		g.fx = append(g.fx, protocol.Fx{Kind: "shake", X: p.Ch.X, Y: p.Ch.Y, R: 0.3})
		return 0
	}
	a.Stamina -= raw * blockDrain
	a.RestT = 0
	if a.Stamina <= 0 {
		a.Blocking = false
		a.T = 1.0
		p.anim("guardbreak")
		g.addFx("hurt", p.Ch.X, p.Ch.Y, "GUARD BROKEN")
		return raw * 0.6
	}
	g.addFx("hurt", p.Ch.X, p.Ch.Y, "blocked")
	return raw * 0.2
}

func (g *Game) actionView(p *Player) (float64, float64, int) {
	return math.Round(p.A.Stamina), math.Round(p.maxStamina()), p.A.Combo
}

var _ = fmt.Sprint
