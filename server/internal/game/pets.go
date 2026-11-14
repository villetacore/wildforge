package game

import (
	"fmt"
	"math"
	"math/rand/v2"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

const (
	petIDBase   = int64(1) << 40
	maxPets     = 8
	petLeash    = 14.0
	petResummon = 20.0
)

// PetEnt is a summoned pet following its owner around the world.
type PetEnt struct {
	ID        int64
	Owner     *Player
	Index     int // into Owner.Ch.Pets
	X, Y      float64
	HP, MaxHP float64
	Dmg       float64
	Speed     float64
	AttackCD  float64
	AbilityCD float64
	Target    int64
	Statuses  []Status
}

func petXPNeeded(level int) int { return int(30 * math.Pow(float64(level), 1.5)) }

func (g *Game) petData(pe *PetEnt) *PetData {
	if pe.Index < 0 || pe.Index >= len(pe.Owner.Ch.Pets) {
		return nil
	}
	return &pe.Owner.Ch.Pets[pe.Index]
}

func (g *Game) activePetAbility(c *Character) string {
	if c.ActivePet >= 0 && c.ActivePet < len(c.Pets) {
		return c.Pets[c.ActivePet].Ability
	}
	return ""
}

func (g *Game) petStats(p *Player, d *PetData) (hp, dmg float64) {
	lv := float64(d.Level - 1)
	hp = d.BaseHP * (1 + 0.14*lv) * (1 + p.S.PetHP)
	dmg = d.BaseDmg * (1 + 0.16*lv) * (1 + p.S.Pet)
	return
}

func newPetFromDef(def *content.PetDef) PetData {
	return PetData{Kind: def.ID, Name: def.Name, Level: 1, Model: def.Model, Texture: def.Texture, Color: def.Color,
		Scale: def.Scale, Fly: def.Fly, BaseHP: def.HP, BaseDmg: def.Dmg, Speed: def.Speed, Ability: def.Ability}
}

func (g *Game) adoptPet(p *Player, d PetData) bool {
	if len(p.Ch.Pets) >= maxPets {
		p.system("You already have the maximum number of pets. Release one first (H).")
		return false
	}
	p.Ch.Pets = append(p.Ch.Pets, d)
	p.system(fmt.Sprintf("%s joined you! Press H to manage your pets.", d.Name))
	g.summonPet(p, len(p.Ch.Pets)-1)
	return true
}

func (g *Game) dismissPet(p *Player) {
	if p.Pet != nil {
		delete(g.pets, p.Pet.ID)
		p.Pet = nil
	}
	p.Ch.ActivePet = -1
}

func (g *Game) summonPet(p *Player, idx int) {
	if idx < 0 || idx >= len(p.Ch.Pets) {
		return
	}
	if p.PetCD > 0 {
		p.system(fmt.Sprintf("Your pets are still recovering (%ds).", int(p.PetCD)))
		return
	}
	g.dismissPet(p)
	d := &p.Ch.Pets[idx]
	hp, dmg := g.petStats(p, d)
	if d.HP <= 0 || d.HP > hp {
		d.HP = hp
	}
	pe := &PetEnt{ID: petIDBase + g.newID(), Owner: p, Index: idx, X: p.Ch.X + 0.8, Y: p.Ch.Y + 0.8, HP: d.HP, MaxHP: hp, Dmg: dmg, Speed: d.Speed}
	if !g.world.Free(pe.X, pe.Y) {
		pe.X, pe.Y = p.Ch.X, p.Ch.Y
	}
	g.pets[pe.ID] = pe
	p.Pet = pe
	p.Ch.ActivePet = idx
}

func (g *Game) damagePet(pe *PetEnt, raw float64, src *Enemy) {
	dmg := math.Max(1, math.Round(raw*randRange(0.85, 1.15)))
	pe.HP -= dmg
	g.addFx("hurt", pe.X, pe.Y, fmt.Sprintf("-%d", int(dmg)))
	if d := g.petData(pe); d != nil {
		d.HP = pe.HP
	}
	if pe.HP > 0 {
		return
	}
	p := pe.Owner
	name := "Your pet"
	if d := g.petData(pe); d != nil {
		name = d.Name
		d.HP = 0
	}
	delete(g.pets, pe.ID)
	p.Pet = nil
	p.PetCD = petResummon
	for _, e := range g.enemies {
		if e.PetTarget == pe.ID {
			e.PetTarget, e.TauntT = 0, 0
		}
	}
	p.system(name + " fainted! It can be summoned again in 20 seconds.")
}

func (g *Game) petGainXP(p *Player, xp int) {
	if p.Pet == nil {
		return
	}
	d := g.petData(p.Pet)
	if d == nil {
		return
	}
	d.XP += int(float64(xp) * (1 + p.S.PetXP))
	for d.XP >= petXPNeeded(d.Level) && d.Level < p.Ch.Level+2 {
		d.XP -= petXPNeeded(d.Level)
		d.Level++
		hp, dmg := g.petStats(p, d)
		p.Pet.MaxHP, p.Pet.HP, p.Pet.Dmg = hp, hp, dmg
		d.HP = hp
		g.addFx("level", p.Pet.X, p.Pet.Y, fmt.Sprintf("%s level %d!", d.Name, d.Level))
	}
}

// tickPets: follow the owner, fight what the owner fights, use abilities.
func (g *Game) tickPets() {
	for _, pe := range g.pets {
		p := pe.Owner
		d := g.petData(pe)
		if d == nil {
			delete(g.pets, pe.ID)
			p.Pet = nil
			continue
		}
		g.tickStatuses(&pe.Statuses)
		pe.AttackCD -= dt
		pe.AbilityCD -= dt
		hp, dmg := g.petStats(p, d)
		pe.MaxHP, pe.Dmg = hp, dmg
		if dist(pe.X, pe.Y, p.Ch.X, p.Ch.Y) > 25 {
			pe.X, pe.Y, pe.Target = p.Ch.X, p.Ch.Y, 0
		}
		// Pick a fight: owner's target, else whatever is attacking the owner.
		if _, ok := g.enemies[pe.Target]; !ok {
			pe.Target = 0
		}
		if e, ok := g.enemies[p.Target]; ok && dist(e.X, e.Y, p.Ch.X, p.Ch.Y) < petLeash && e.Target == p.ID {
			pe.Target = p.Target
		}
		if pe.Target == 0 {
			for id, e := range g.enemies {
				if e.Target == p.ID && dist(e.X, e.Y, p.Ch.X, p.Ch.Y) < 9 {
					pe.Target = id
					break
				}
			}
		}
		e, fighting := g.enemies[pe.Target]
		if fighting && dist(e.X, e.Y, p.Ch.X, p.Ch.Y) > petLeash {
			fighting, pe.Target = false, 0
		}
		if !fighting {
			pe.HP = math.Min(pe.MaxHP, pe.HP+pe.MaxHP*0.04*dt)
			d.HP = pe.HP
			// Trail behind the owner.
			tx, ty := p.Ch.X-p.DirX*1.2+0.6, p.Ch.Y-p.DirY*1.2+0.6
			if dd := dist(pe.X, pe.Y, tx, ty); dd > 0.6 {
				s := math.Min(pe.Speed*math.Max(1, dd/3)*dt, dd)
				pe.X, pe.Y = g.petStep(pe, d, (tx-pe.X)/dd*s, (ty-pe.Y)/dd*s)
			}
			continue
		}
		dd := dist(pe.X, pe.Y, e.X, e.Y)
		reach := 0.9 + 0.3*e.Scale
		if dd > reach {
			s := pe.Speed * dt / dd
			pe.X, pe.Y = g.petStep(pe, d, (e.X-pe.X)*s, (e.Y-pe.Y)*s)
			continue
		}
		if pe.AttackCD <= 0 {
			pe.AttackCD = 1.1
			g.petHit(pe, pe.Target, e, pe.Dmg*randRange(0.85, 1.15), "phys")
		}
		if ab := g.pack.Ability(d.Ability); ab != nil && ab.CD > 0 && pe.AbilityCD <= 0 {
			if _, alive := g.enemies[pe.Target]; !alive {
				continue
			}
			pe.AbilityCD = ab.CD
			g.addFx("status", pe.X, pe.Y, ab.Name+"!")
			if ab.Taunt {
				for _, o := range g.enemies {
					if dist(o.X, o.Y, pe.X, pe.Y) < 5 {
						o.PetTarget, o.TauntT = pe.ID, 4
					}
				}
				continue
			}
			if ab.Radius > 0 {
				g.fx = append(g.fx, protocol.Fx{Kind: "nova", X: pe.X, Y: pe.Y, R: ab.Radius, Color: "#ffd060"})
				for oid, o := range g.enemies {
					if dist(o.X, o.Y, pe.X, pe.Y) <= ab.Radius {
						if ab.Status != "" {
							g.addStatus(&o.Statuses, ab.Status, 1+float64(d.Level)*0.3)
						}
						g.petHit(pe, oid, o, pe.Dmg*ab.Power, "phys")
					}
				}
				continue
			}
			if ab.Status != "" {
				g.addStatus(&e.Statuses, ab.Status, 1+float64(d.Level)*0.3)
			}
			g.petHit(pe, pe.Target, e, pe.Dmg*ab.Power, "phys")
		}
	}
}

func (g *Game) petStep(pe *PetEnt, d *PetData, dx, dy float64) (float64, float64) {
	if d.Fly {
		nx, ny := pe.X+dx, pe.Y+dy
		if xi, yi := int(nx), int(ny); g.world.InBounds(xi, yi) && !g.world.Underground(nx, ny) == !g.world.Underground(pe.X, pe.Y) {
			return nx, ny
		}
	}
	return g.world.Step(pe.X, pe.Y, dx, dy)
}

func (g *Game) petHit(pe *PetEnt, id int64, e *Enemy, dmg float64, school string) {
	p := pe.Owner
	if p.S.PetLeech > 0 {
		g.healPlayer(p, dmg*p.S.PetLeech, false)
	}
	if e.Target == 0 {
		e.Target = p.ID
	}
	g.damageEnemy(p, id, e, dmg, school, false, false)
}

// tame turns a weakened beast into a pet.
func (g *Game) tame(p *Player) {
	id, e := g.pickTarget(p, 3)
	if e == nil {
		p.system("Get close to a weakened beast to tame it.")
		return
	}
	if !e.Fam.Tameable || e.Boss || e.ContractID != 0 {
		p.system(e.Name + " cannot be tamed.")
		return
	}
	threshold := 0.25
	lvlBonus := 0
	if p.S.Tame {
		threshold, lvlBonus = 0.4, 3
	}
	if e.Level > p.Ch.Level+lvlBonus {
		p.system(e.Name + " is too strong for you to tame.")
		return
	}
	if e.HP > e.MaxHP*threshold {
		p.system(fmt.Sprintf("Weaken it first (below %d%% health).", int(threshold*100)))
		return
	}
	if !p.Ch.inv(g.pack).Remove("tame_rope", 1) {
		p.system("You need a Taming Rope (sold by merchants and stablemasters).")
		return
	}
	if rand.Float64() > 0.45+0.5*(1-e.HP/e.MaxHP/threshold) {
		p.system("The beast breaks free from the rope!")
		e.HP = math.Min(e.MaxHP, e.HP+e.MaxHP*0.1)
		return
	}
	ability := map[string]string{"wolf": "rend", "bear": "maul", "boar": "taunt", "spider": "venom", "bat": "peck"}[e.Fam.ID]
	if ability == "" {
		ability = "rend"
	}
	delete(g.enemies, id)
	if e.Respawns {
		g.enemyTimers = append(g.enemyTimers, g.pack.World.EnemyRespawn)
	}
	g.addFx("level", e.X, e.Y, "Tamed!")
	g.adoptPet(p, PetData{
		Kind: "tamed:" + e.Fam.ID, Name: e.Name, Level: max(1, e.Level-1), Model: e.Fam.Model, Texture: e.Fam.Texture,
		Color: e.Color, Scale: math.Min(1.1, e.Scale*0.8), BaseHP: e.Fam.HP * 1.4, BaseDmg: e.Fam.Dmg * 1.1, Speed: 6, Ability: ability,
	})
}

func (g *Game) petViews(near func(x, y float64) bool) []protocol.Pet {
	out := []protocol.Pet{}
	for _, pe := range g.pets {
		if !near(pe.X, pe.Y) {
			continue
		}
		d := g.petData(pe)
		if d == nil {
			continue
		}
		out = append(out, protocol.Pet{ID: pe.ID, Owner: pe.Owner.ID, Name: d.Name, Model: d.Model, Tex: d.Texture, Color: d.Color,
			Scale: d.Scale, Fly: d.Fly, X: pe.X, Y: pe.Y, HP: math.Round(pe.HP), MaxHP: math.Round(pe.MaxHP), Level: d.Level})
	}
	return out
}
