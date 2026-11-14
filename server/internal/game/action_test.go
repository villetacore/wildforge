package game

import (
	"testing"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

// dummy places a tough, harmless enemy right in front of the player.
func dummy(g *Game, p *Player) (int64, *Enemy) {
	fam := g.pack.Family("wolf")
	e := g.makeEnemy(fam, g.pack.Element("none"), 1, 1, nil, p.Ch.X, p.Ch.Y+1)
	e.MaxHP, e.HP, e.Armor = 1e6, 1e6, 0
	id := g.newID()
	g.enemies[id] = e
	return id, e
}

func ticks(g *Game, p *Player, n int) {
	for range n {
		g.tickAction(p)
	}
}

func TestComboChainAndFinisher(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.X, p.Ch.Y = 300.5, 230.5
	_, e := dummy(g, p)
	var anims []string
	press := func(kind string) {
		g.input(p, protocol.ClientMsg{T: "attack", Opt: kind, DX: 0, DY: 1})
		anims = append(anims, p.A.Anim)
		ticks(g, p, int(p.A.T*TickRate)+1)
	}
	press("light")
	press("light")
	press("heavy")
	if anims[0] != "light1" || anims[1] != "light2" || anims[2] != "finisher" {
		t.Fatalf("combo = %v", anims)
	}
	if e.HP >= 1e6 {
		t.Fatal("attacks did not land")
	}
	if e.StaggerT <= 0 {
		t.Fatal("finisher should stagger")
	}
	// Stamina is spent and gates attacks.
	p.A.Stamina = -1
	before := p.A.Seq
	ticks(g, p, 30)
	g.input(p, protocol.ClientMsg{T: "attack", DX: 0, DY: 1})
	if p.A.Seq != before && p.A.Stamina < 0 {
		t.Fatal("attacked without stamina")
	}
}

func TestAttackHitsOnlyInFront(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.X, p.Ch.Y = 300.5, 230.5
	_, e := dummy(g, p)                                        // south of the player (+y)
	g.input(p, protocol.ClientMsg{T: "attack", DX: 0, DY: -1}) // swing north
	ticks(g, p, 20)
	if e.HP < 1e6 {
		t.Fatal("hit an enemy behind the swing")
	}
}

func TestDodgeIFramesAndParry(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.X, p.Ch.Y = 300.5, 230.5
	_, e := dummy(g, p)
	e.Dmg = 50
	hp := p.Ch.HP
	g.input(p, protocol.ClientMsg{T: "dodge", DX: 1, DY: 0})
	g.damagePlayer(p, 50, e)
	if p.Ch.HP != hp {
		t.Fatal("rolled through the blow but still took damage")
	}
	ticks(g, p, 40)
	// Raise the block just before the blow: a parry staggers the attacker.
	g.input(p, protocol.ClientMsg{T: "block", Opt: "on"})
	p.A.FaceX, p.A.FaceY = 0, 1
	g.damagePlayer(p, 50, e)
	if p.Ch.HP != hp || e.StaggerT <= 0 || !e.Riposte {
		t.Fatalf("parry failed: hp %v stagger %v", p.Ch.HP, e.StaggerT)
	}
	// A late block only reduces damage and costs stamina.
	ticks(g, p, 20)
	st := p.A.Stamina
	g.damagePlayer(p, 50, e)
	if p.Ch.HP >= hp || p.Ch.HP < hp-20 || p.A.Stamina >= st {
		t.Fatalf("block: hp %v -> %v, stamina %v -> %v", hp, p.Ch.HP, st, p.A.Stamina)
	}
}

func TestEnemyTelegraphsBeforeHitting(t *testing.T) {
	g, p := newTestGame(t)
	p.Ch.X, p.Ch.Y = 300.5, 230.5
	for id := range g.enemies {
		delete(g.enemies, id)
	}
	_, e := dummy(g, p)
	e.Dmg, e.Target = 30, p.ID
	hp := p.Ch.HP
	for i := 0; i < 20 && e.Windup <= 0; i++ {
		g.tickEnemies()
	}
	if e.Windup <= 0 || e.Anim != "windup" || p.Ch.HP != hp {
		t.Fatalf("expected a wind-up first: windup %v anim %q", e.Windup, e.Anim)
	}
	for range 20 {
		g.tickEnemies()
	}
	if p.Ch.HP >= hp {
		t.Fatal("the blow never landed")
	}
}
