package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"sort"
	"strings"

	"github.com/villetacore/wildforge/server/internal/content"
)

// Display names and formatting for stat keys.
var statLabels = map[string]string{
	"atk": "Attack", "def": "Defense", "hp": "Health", "mana": "Mana", "str": "Strength", "dex": "Dexterity",
	"int": "Intellect", "vit": "Vitality", "crit": "% Critical", "sp": "Spell Power", "speed": "% Speed",
	"lifesteal": "% Lifesteal", "regen": "Regen/s", "mana_regen": "Mana Regen/s", "thorns": "Thorns",
	"fire": "Fire Damage", "frost": "Frost Damage", "poison": "Poison Damage", "shadow": "Shadow Damage",
	"storm": "Storm Damage", "holy": "Holy Damage", "mf": "% Magic Find", "gold": "% Gold Find",
	"dodge": "% Dodge", "cdr": "% Cooldown Reduction", "pet": "% Pet Damage",
}

var statOrder = []string{"atk", "def", "sp", "hp", "mana", "str", "dex", "int", "vit", "crit", "dodge", "speed",
	"lifesteal", "regen", "mana_regen", "thorns", "fire", "frost", "poison", "shadow", "storm", "holy", "cdr", "mf", "gold", "pet"}

func weighted[T any](list []T, weight func(*T) float64) *T {
	total := 0.0
	for i := range list {
		total += math.Max(0, weight(&list[i]))
	}
	if total <= 0 {
		return &list[rand.IntN(len(list))]
	}
	r := rand.Float64() * total
	for i := range list {
		r -= math.Max(0, weight(&list[i]))
		if r <= 0 {
			return &list[i]
		}
	}
	return &list[len(list)-1]
}

func slotAllowed(slots []string, slot string) bool {
	for _, s := range slots {
		if s == "any" || s == slot {
			return true
		}
	}
	return false
}

// GearSpec pins parts of a generated item; empty fields are rolled.
type GearSpec struct {
	Base, Mat    string
	Quality      int // -1 = roll
	Rarity       int // -1 = roll
	MinRarity    int
	QualityBoost float64
	RarityBoost  float64 // > 0 shifts weight toward rarer results
	Crafter      string
}

// genGear rolls a new item for the given item level.
func (g *Game) genGear(level int, spec GearSpec) *content.Gear {
	t := &g.pack.Gear
	var base *content.GearBase
	if spec.Base != "" {
		base = g.pack.Base(spec.Base)
	}
	if base == nil {
		var cands []content.GearBase
		for _, b := range t.Bases {
			if b.Level <= level+2 {
				cands = append(cands, b)
			}
		}
		base = g.pack.Base(cands[rand.IntN(len(cands))].ID)
	}
	var mat *content.Material
	if spec.Mat != "" {
		mat = g.pack.Material(spec.Mat)
	}
	if mat == nil || mat.Kind != base.Mats {
		var cands []content.Material
		for _, m := range t.Materials {
			if m.Kind == base.Mats && m.Level <= level+1 {
				cands = append(cands, m)
			}
		}
		if len(cands) == 0 {
			for _, m := range t.Materials {
				if m.Kind == base.Mats {
					cands = append(cands, m)
					break
				}
			}
		}
		// Prefer materials close to the item level.
		mat = g.pack.Material(weighted(cands, func(m *content.Material) float64 {
			return 1 / (1 + math.Abs(float64(level-m.Level))*0.4)
		}).ID)
	}
	quality := spec.Quality
	if quality < 0 {
		q := weighted(t.Qualities, func(q *content.Quality) float64 {
			return q.Weight * math.Pow(q.Mult, spec.QualityBoost*6)
		})
		for i := range t.Qualities {
			if t.Qualities[i].ID == q.ID {
				quality = i
			}
		}
	}
	rarity := spec.Rarity
	if rarity < 0 {
		r := weighted(t.Rarities, func(r *content.Rarity) float64 {
			return r.Weight * math.Pow(1+float64(r.Affixes), spec.RarityBoost*1.5)
		})
		for i := range t.Rarities {
			if t.Rarities[i].ID == r.ID {
				rarity = i
			}
		}
	}
	rarity = max(rarity, spec.MinRarity)
	gear := &content.Gear{Base: base.ID, Mat: mat.ID, Quality: quality, Rarity: rarity, Level: max(1, level), Crafter: spec.Crafter}
	// Affixes: at most two prefixes and two suffixes, never repeated.
	need := t.Rarities[rarity].Affixes
	perm := rand.Perm(len(t.Affixes))
	pre, suf := 0, 0
	for _, i := range perm {
		if len(gear.Affixes) >= need {
			break
		}
		a := t.Affixes[i]
		if a.Level > level || !slotAllowed(a.Slots, base.Slot) {
			continue
		}
		if a.Pos == "prefix" && pre < 2 {
			pre++
		} else if a.Pos == "suffix" && suf < 2 {
			suf++
		} else {
			continue
		}
		gear.Affixes = append(gear.Affixes, a.ID)
	}
	if t.Rarities[rarity].ID == "legendary" && len(t.LegendNames) > 0 {
		gear.Legend = t.LegendNames[rand.IntN(len(t.LegendNames))]
	}
	g.refreshGear(gear)
	return gear
}

// refreshGear derives name, colors, stats, value and tooltip lines.
func (g *Game) refreshGear(gr *content.Gear) {
	t := &g.pack.Gear
	base := g.pack.Base(gr.Base)
	mat := g.pack.Material(gr.Mat)
	if base == nil || mat == nil {
		gr.Name, gr.Stats, gr.Lines = "Broken Relic", content.Stats{}, []string{"Its power has faded."}
		return
	}
	gr.Quality = min(max(gr.Quality, 0), len(t.Qualities)-1)
	gr.Rarity = min(max(gr.Rarity, 0), len(t.Rarities)-1)
	q := t.Qualities[gr.Quality]
	r := t.Rarities[gr.Rarity]
	lvl := float64(gr.Level)
	stats := content.Stats{}
	mult := mat.Tier * q.Mult * r.Mult * (1 + lvl*0.05)
	for k, v := range base.Stats {
		if v > 0 {
			stats[k] += math.Round(v*mult*10) / 10
		} else {
			stats[k] += v
		}
	}
	var prefixes, suffixes []string
	for _, id := range gr.Affixes {
		a := g.pack.Affix(id)
		if a == nil {
			continue
		}
		stats.Add(a.Stats, 1+lvl*0.08)
		if a.Pos == "prefix" {
			prefixes = append(prefixes, a.Name)
		} else {
			suffixes = append(suffixes, a.Name)
		}
	}
	if e := g.pack.EnchantDef(gr.Enchant); e != nil {
		stats.Add(e.Stats, 1+lvl*0.05)
	}
	for k, v := range stats {
		if k == "crit" || k == "speed" || k == "dodge" || k == "cdr" || k == "lifesteal" || k == "regen" || k == "mana_regen" {
			stats[k] = math.Round(v*10) / 10
		} else {
			stats[k] = math.Round(v)
		}
		if stats[k] == 0 {
			delete(stats, k)
		}
	}
	gr.Stats = stats
	gr.Slot = base.Slot
	gr.Model = base.Model
	gr.Color = r.Color
	gr.Tint = mat.Color

	parts := []string{}
	if q.Name != "" {
		parts = append(parts, q.Name)
	}
	if len(prefixes) > 0 {
		parts = append(parts, prefixes[0])
	}
	parts = append(parts, mat.Name, base.Name)
	name := strings.Join(parts, " ")
	if len(suffixes) > 0 {
		name += " " + suffixes[0]
	}
	if gr.Legend != "" {
		name = gr.Legend + ", " + mat.Name + " " + base.Name
	}
	if gr.Custom != "" {
		name = "\"" + gr.Custom + "\" " + mat.Name + " " + base.Name
	}
	gr.Name = name

	lines := []string{fmt.Sprintf("%s %s · item level %d", r.Name, strings.ToLower(slotName(base.Slot)), gr.Level)}
	keys := make([]string, 0, len(stats))
	for k := range stats {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(a, b int) bool { return statRank(keys[a]) < statRank(keys[b]) })
	for _, k := range keys {
		lines = append(lines, fmtStat(k, stats[k]))
	}
	if e := g.pack.EnchantDef(gr.Enchant); e != nil {
		lines = append(lines, "Enchanted: "+e.Name)
	}
	if gr.Crafter != "" {
		lines = append(lines, "Forged by "+gr.Crafter)
	}
	gr.Lines = lines
	value := 0.0
	for _, v := range stats {
		value += math.Abs(v)
	}
	gr.Value = int(value*2*(1+float64(gr.Rarity)*0.6)) + gr.Level*3
}

func statRank(k string) int {
	for i, s := range statOrder {
		if s == k {
			return i
		}
	}
	return 100
}

func fmtStat(k string, v float64) string {
	label := statLabels[k]
	if label == "" {
		label = k
	}
	sign := "+"
	if v < 0 {
		sign = ""
	}
	if strings.HasPrefix(label, "%") {
		return fmt.Sprintf("%s%g%s", sign, v, label)
	}
	return fmt.Sprintf("%s%g %s", sign, v, label)
}

func slotName(s string) string {
	switch s {
	case "offhand":
		return "Off-hand"
	case "head":
		return "Head"
	case "chest":
		return "Chest"
	case "legs":
		return "Legs"
	case "hands":
		return "Hands"
	case "feet":
		return "Feet"
	case "ring":
		return "Ring"
	case "amulet":
		return "Amulet"
	}
	return "Weapon"
}

// stackName is the display name of any inventory stack.
func (g *Game) stackName(s *content.Stack) string {
	if s == nil {
		return "-"
	}
	if s.G != nil {
		return s.G.Name
	}
	if d := g.pack.Item(s.Item); d != nil {
		return d.Name
	}
	return s.Item
}

// stackValue is the base gold value of one unit.
func (g *Game) stackValue(s *content.Stack) int {
	if s.G != nil {
		return s.G.Value
	}
	if d := g.pack.Item(s.Item); d != nil {
		return d.Price
	}
	return 0
}

// equipSlot returns the slot a stack goes into, or "".
func (g *Game) equipSlot(s *content.Stack) string {
	if s.G != nil {
		return s.G.Slot
	}
	if d := g.pack.Item(s.Item); d != nil {
		switch d.Type {
		case "weapon":
			return "weapon"
		case "armor":
			return "chest"
		}
	}
	return ""
}
