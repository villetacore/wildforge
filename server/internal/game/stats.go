package game

import (
	"math"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

// Derived combat numbers for a player, recomputed every tick from level,
// attributes, equipment, perks, statuses and mood.
type PStats struct {
	MaxHP, MaxMana    float64
	Atk, Def, SP      float64
	Crit, CritMult    float64
	Lifesteal, Dodge  float64
	Speed             float64
	Regen, ManaRegen  float64
	Thorns, CDR, MF   float64
	Gold, XP, Rep     float64
	Price, Potion     float64
	Pet, PetHP        float64
	PetLeech, PetXP   float64
	Cleave            int
	PoisonHit, Opener float64
	SpellStatus       float64
	DiseaseResist     float64
	CombatRegen       bool
	NoDeathPenalty    bool
	Charm             bool
	MoodKeep          float64
	Contract          float64
	Tame              bool
	Elem              map[string]float64
	Attr              [4]float64
	Raw               content.Stats
}

var elements = []string{"fire", "frost", "poison", "shadow", "storm", "holy"}

// itemStats returns the stat bonuses of an equipped stack.
func (g *Game) itemStats(s *content.Stack) content.Stats {
	if s == nil {
		return nil
	}
	if s.G != nil {
		return s.G.Stats
	}
	def := g.pack.Item(s.Item)
	if def == nil {
		return nil
	}
	return content.Stats{"atk": float64(def.Attack), "def": float64(def.Defense)}
}

func (g *Game) computeStats(c *Character) PStats {
	raw := content.Stats{}
	for _, slot := range slots {
		raw.Add(g.itemStats(c.Equip[slot]), 1)
	}
	for _, id := range c.Perks {
		if p := g.pack.Perk(id); p != nil {
			raw.Add(p.Stats, 1)
		}
	}
	for _, st := range c.Statuses {
		if d := g.pack.Status(st.ID); d != nil {
			raw.Add(d.Stats, 1)
			raw["xp_status"] += d.XP * 100
		}
	}
	if g.activePetAbility(c) == "luck" {
		raw["mf"] += 20
	}
	var s PStats
	s.Raw = raw
	for i := range s.Attr {
		s.Attr[i] = math.Max(0, float64(c.Attrs[i])+raw[attrKeys[i]])
	}
	str, dex, intl, vit := s.Attr[0], s.Attr[1], s.Attr[2], s.Attr[3]
	lvl := float64(c.Level - 1)
	s.MaxHP = (100 + 16*lvl + vit*10 + raw["hp"]) * (1 + raw["hp_pct"]/100)
	s.MaxMana = (50 + 8*lvl + intl*8 + raw["mana"]) * (1 + raw["mana_pct"]/100)
	s.Atk = (4 + 2.5*lvl + str*1.5 + raw["atk"]) * (1 + raw["atk_pct"]/100)
	s.Def = math.Floor(lvl/2) + raw["def"] + vit*0.3
	s.SP = (lvl + intl*2 + raw["sp"]) * (1 + raw["sp_pct"]/100)
	s.Crit = math.Min(75, 5+dex*0.4+raw["crit"])
	s.CritMult = 2 + raw["crit_mult"]
	s.Lifesteal = raw["lifesteal"] / 100
	s.Dodge = math.Min(50, raw["dodge"]+dex*0.15) / 100
	s.Speed = playerSpeed * math.Max(0.4, 1+raw["speed"]/100+dex*0.002)
	s.Regen = 0.6 + vit*0.1 + raw["regen"]
	s.ManaRegen = (1 + intl*0.08 + raw["mana_regen"]) * (1 + raw["mana_regen_pct"]/100)
	s.Thorns = raw["thorns"]
	s.CDR = math.Min(50, raw["cdr"]) / 100
	s.MF = raw["mf"] / 100
	s.Gold = raw["gold"] / 100
	s.XP = (raw["xp"] + raw["xp_status"]) / 100
	s.Rep = raw["rep"] / 100
	s.Price = math.Min(40, raw["price"]) / 100
	s.Potion = raw["potion"] / 100
	s.Pet = raw["pet"] / 100
	s.PetHP = raw["pet_hp"] / 100
	s.PetLeech = raw["pet_leech"] / 100
	s.PetXP = raw["pet_xp"] / 100
	s.Cleave = int(raw["cleave"])
	s.PoisonHit = raw["poison_hit"] / 100
	s.Opener = raw["opener"] / 100
	s.SpellStatus = raw["spell_status"] / 100
	s.DiseaseResist = math.Min(100, raw["disease_resist"]) / 100
	s.CombatRegen = raw["combat_regen"] > 0
	s.NoDeathPenalty = raw["no_death_penalty"] > 0
	s.Charm = raw["charm"] > 0
	s.MoodKeep = raw["mood_keep"] / 100
	s.Contract = raw["contract"] / 100
	s.Tame = raw["tame"] > 0
	s.Elem = map[string]float64{}
	for _, e := range elements {
		s.Elem[e] = raw[e]
	}
	// Mood: content characters fight and learn better, miserable ones worse.
	switch {
	case c.Mood >= 80:
		s.XP += 0.1
		s.Regen += 1
	case c.Mood < 25:
		s.Atk *= 0.9
		s.XP -= 0.1
	}
	return s
}

// ---- statuses (shared by players and enemies)

// addStatus applies or refreshes an effect; returns false if resisted.
func (g *Game) addStatus(list *[]Status, id string, mag float64) bool {
	def := g.pack.Status(id)
	if def == nil {
		return false
	}
	for i := range *list {
		s := &(*list)[i]
		if s.ID == id {
			if s.T >= 0 {
				s.T = math.Max(s.T, def.Dur)
			}
			s.Mag = math.Max(s.Mag, mag)
			return true
		}
	}
	*list = append(*list, Status{ID: id, T: def.Dur, Mag: math.Max(mag, 1)})
	return true
}

func hasStatus(list []Status, id string) bool {
	for _, s := range list {
		if s.ID == id {
			return true
		}
	}
	return false
}

func removeStatuses(list *[]Status, keep func(Status) bool) int {
	out := (*list)[:0]
	removed := 0
	for _, s := range *list {
		if keep(s) {
			out = append(out, s)
		} else {
			removed++
		}
	}
	*list = out
	return removed
}

// statusMods sums the modifiers of active statuses.
type statusMods struct {
	stun        bool
	slow        float64
	taken       float64
	dealt       float64
	dot         float64
	hot         float64
	absorbIndex int
}

func (g *Game) mods(list []Status) statusMods {
	m := statusMods{taken: 1, dealt: 1, absorbIndex: -1}
	for i, s := range list {
		d := g.pack.Status(s.ID)
		if d == nil {
			continue
		}
		if d.Stun {
			m.stun = true
		}
		m.slow = math.Max(m.slow, d.Slow)
		if d.Taken > 0 {
			m.taken *= d.Taken
		}
		if d.Dealt > 0 {
			m.dealt *= d.Dealt
		}
		m.dot += d.Dot * s.Mag
		m.hot += d.Hot
		if d.Absorb > 0 {
			m.absorbIndex = i
		}
	}
	return m
}

// tickStatuses counts down durations. Returns the damage-over-time dealt this
// tick (before resistances) and heal-over-time as a fraction of max HP.
func (g *Game) tickStatuses(list *[]Status) (dot, hot float64) {
	m := g.mods(*list)
	removeStatuses(list, func(s Status) bool {
		if s.T < 0 {
			return true
		}
		s.T -= dt
		return s.T > 0
	})
	for i := range *list {
		if (*list)[i].T > 0 {
			(*list)[i].T -= dt
		}
	}
	return m.dot * dt, m.hot * dt
}

// absorb lets a shield soak damage; returns what is left.
func (g *Game) absorb(list *[]Status, dmg float64) float64 {
	m := g.mods(*list)
	if m.absorbIndex < 0 {
		return dmg
	}
	s := &(*list)[m.absorbIndex]
	def := g.pack.Status(s.ID)
	if s.Mag <= 1 {
		s.Mag = def.Absorb
	}
	soak := math.Min(s.Mag, dmg)
	s.Mag -= soak
	if s.Mag <= 0 {
		s.T = 0.001
	}
	return dmg - soak
}

func (g *Game) statusViews(list []Status) []protocol.StatusView {
	out := make([]protocol.StatusView, 0, len(list))
	for _, s := range list {
		if d := g.pack.Status(s.ID); d != nil {
			out = append(out, protocol.StatusView{ID: s.ID, Name: d.Name, Color: d.Color, T: math.Round(s.T), Bad: d.Bad})
		}
	}
	return out
}
