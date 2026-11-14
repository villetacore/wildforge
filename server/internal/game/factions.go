package game

import (
	"fmt"
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
)

const repMin, repMax = -42000, 42999

// rep returns standing with a faction. Hostile factions start hated.
func (g *Game) rep(p *Player, faction string) int {
	if v, ok := p.Ch.Rep[faction]; ok {
		return v
	}
	if f := g.pack.Faction(faction); f != nil && f.Hostile {
		return -5000
	}
	return 0
}

func (g *Game) repRankIndex(p *Player, faction string) int {
	v := g.rep(p, faction)
	ranks := g.pack.FactionsT.Ranks
	idx := 0
	for i, r := range ranks {
		if v >= r.Min {
			idx = i
		}
	}
	return idx
}

func (g *Game) repRank(p *Player, faction string) content.RepRank {
	if g.pack.Faction(faction) == nil {
		return content.RepRank{Name: "Neutral", Price: 1}
	}
	return g.pack.FactionsT.Ranks[g.repRankIndex(p, faction)]
}

func (g *Game) factionName(id string) string {
	if f := g.pack.Faction(id); f != nil {
		return f.Name
	}
	return id
}

func (g *Game) repLine(p *Player, faction string) string {
	r := g.repRank(p, faction)
	return fmt.Sprintf("Standing with the %s: [color=%s]%s[/color] (%d)", g.factionName(faction), r.Color, r.Name, g.rep(p, faction))
}

// changeRep adjusts standing. Gains also ripple: allies like you a bit more,
// their enemies a bit less.
func (g *Game) changeRep(p *Player, faction string, amount int) {
	f := g.pack.Faction(faction)
	if f == nil || amount == 0 {
		return
	}
	g.applyRep(p, faction, amount)
	if amount > 0 {
		for _, a := range f.Allies {
			g.applyRep(p, a, amount/4)
		}
		for _, e := range f.Enemies {
			g.applyRep(p, e, -amount/4)
		}
	}
}

func (g *Game) applyRep(p *Player, faction string, amount int) {
	if amount == 0 || g.pack.Faction(faction) == nil {
		return
	}
	if amount > 0 {
		amount = int(math.Round(float64(amount) * (1 + p.S.Rep)))
	}
	before := g.repRankIndex(p, faction)
	v := min(repMax, max(repMin, g.rep(p, faction)+amount))
	p.Ch.Rep[faction] = v
	if after := g.repRankIndex(p, faction); after != before {
		r := g.pack.FactionsT.Ranks[after]
		p.system(fmt.Sprintf("Your standing with the %s is now %s.", g.factionName(faction), r.Name))
	}
}
