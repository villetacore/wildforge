package game

import (
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

var (
	skinNames  = []string{"Fair", "Rosy", "Olive", "Bronze", "Deep brown"}
	hairNames  = []string{"Bald", "Cropped", "Long", "Ponytail", "Mohawk", "Braided"}
	hairColors = []string{"Black", "Brown", "Chestnut", "Blond", "Red", "Grey", "White"}
	beardNames = []string{"Clean-shaven", "Stubble", "Full beard", "Long beard"}
	buildNames = []string{"Slim", "Average", "Broad"}
	eyeNames   = []string{"Brown", "Blue", "Green", "Grey", "Amber"}
	modelNames = []string{"Knight", "Barbarian", "Mage", "Rogue", "Hooded Rogue", "Classic (customisable)"}
	spellKeys  = []string{"R", "F", "G", "T", "Z", "X"}
)

// openMenu shows one of the player's own windows (not tied to an NPC).
func (g *Game) openMenu(p *Player, menu string) { g.menuChoice(p, menu, "", "") }

func (g *Game) menuChoice(p *Player, menu, opt, text string) {
	switch menu {
	case "char":
		g.charMenu(p, opt)
	case "spells":
		g.spellMenu(p, opt)
	case "pets":
		g.petMenu(p, opt, text)
	case "rep":
		g.repMenu(p)
	case "journal":
		g.journalMenu(p, opt)
	case "look":
		g.lookMenu(p, opt)
	}
}

func menuDialog(menu, title, text string) *dlg {
	d := newDialog(nil, menu, title, text)
	return d
}

// ---- character sheet: attributes and perk trees

func (g *Game) charMenu(p *Player, opt string) {
	c := p.Ch
	tree := ""
	switch {
	case strings.HasPrefix(opt, "attr:"):
		i, _ := strconv.Atoi(opt[5:])
		if i >= 0 && i < 4 && c.StatPoints > 0 {
			c.StatPoints--
			c.Attrs[i]++
			p.S = g.computeStats(c)
		}
	case strings.HasPrefix(opt, "tree:"):
		tree = opt[5:]
	case strings.HasPrefix(opt, "perk:"):
		id := opt[5:]
		if pk := g.pack.Perk(id); pk != nil && g.perkAvailable(c, id) {
			c.Perks = append(c.Perks, id)
			c.PerkPoints--
			p.S = g.computeStats(c)
			g.addFx("level", c.X, c.Y, pk.Name+"!")
			tree = pk.Tree
		}
	case opt == "respec":
		cost := 50 * c.Level
		if c.Gold >= cost {
			c.Gold -= cost
			c.StatPoints += c.Attrs[0] + c.Attrs[1] + c.Attrs[2] + c.Attrs[3]
			c.Attrs = [4]int{}
			c.PerkPoints += len(c.Perks)
			c.Perks = nil
			p.S = g.computeStats(c)
		}
	}
	s := p.S
	if tree != "" {
		g.perkTree(p, tree)
		return
	}
	text := fmt.Sprintf("Level %d %s  ·  %d attribute points  ·  %d perk points\n\n", c.Level, p.Name, c.StatPoints, c.PerkPoints)
	text += fmt.Sprintf("Health %d   Mana %d   Attack %d   Defense %d   Spell Power %d\n", int(s.MaxHP), int(s.MaxMana), int(s.Atk), int(s.Def), int(s.SP))
	text += fmt.Sprintf("Critical %.1f%% (×%.1f)   Dodge %.0f%%   Speed %.0f%%   Lifesteal %.0f%%\n", s.Crit, s.CritMult, s.Dodge*100, s.Speed/playerSpeed*100, s.Lifesteal*100)
	text += fmt.Sprintf("Regen %.1f/s   Mana regen %.1f/s   Magic find +%.0f%%   XP bonus %+.0f%%   Mood %d/100\n", s.Regen, s.ManaRegen, s.MF*100, s.XP*100, int(c.Mood))
	if c.Title != "" {
		text += "Title: " + c.Title + "\n"
	}
	d := menuDialog("char", "Character", text)
	d.Sub = "K"
	desc := [4]string{"+1.5 attack", "+0.4% crit, dodge, speed", "+2 spell power, +8 mana", "+10 health, regen"}
	for i := 0; i < 4; i++ {
		d.optx(protocol.Option{ID: fmt.Sprintf("attr:%d", i), Label: fmt.Sprintf("%s %d  (+%d)", attrNames[i], int(s.Attr[i]), c.Attrs[i]), Hint: desc[i], Disabled: c.StatPoints == 0, Color: "#e0d0a0"})
	}
	for _, t := range g.pack.Perks.Trees {
		owned, total := 0, 0
		for _, pk := range g.pack.Perks.Perks {
			if pk.Tree == t.ID {
				total++
				if c.hasPerk(pk.ID) {
					owned++
				}
			}
		}
		d.optx(protocol.Option{ID: "tree:" + t.ID, Label: fmt.Sprintf("Perks: %s (%d/%d)", t.Name, owned, total), Color: t.Color})
	}
	d.optx(protocol.Option{ID: "respec", Label: fmt.Sprintf("Reset attributes and perks (%dg)", 50*c.Level), Disabled: c.Gold < 50*c.Level, Color: "#a0a0a0"})
	g.show(p, d.opt("close", "Close."))
}

func perkLevel(tier int) int { return max(1, (tier-1)*5) }

func (g *Game) perkAvailable(c *Character, id string) bool {
	pk := g.pack.Perk(id)
	if pk == nil || c.hasPerk(id) || c.PerkPoints <= 0 || c.Level < perkLevel(pk.Tier) {
		return false
	}
	inTree := 0
	for _, o := range c.Perks {
		if op := g.pack.Perk(o); op != nil && op.Tree == pk.Tree {
			inTree++
		}
	}
	return inTree >= pk.Tier-1
}

func (g *Game) perkTree(p *Player, tree string) {
	c := p.Ch
	name := tree
	for _, t := range g.pack.Perks.Trees {
		if t.ID == tree {
			name = t.Name
		}
	}
	d := menuDialog("char", name+" perks", fmt.Sprintf("Perk points: %d. Tier N needs level %d/%d/%d/%d and N−1 perks from this tree.", c.PerkPoints, perkLevel(1), perkLevel(2), perkLevel(3), perkLevel(4)))
	for _, pk := range g.pack.Perks.Perks {
		if pk.Tree != tree {
			continue
		}
		o := protocol.Option{ID: "perk:" + pk.ID, Hint: pk.Desc}
		switch {
		case c.hasPerk(pk.ID):
			o.Label, o.Color, o.Disabled = fmt.Sprintf("✔ %s — %s", pk.Name, pk.Desc), "#8aff8a", true
		case g.perkAvailable(c, pk.ID):
			o.Label, o.Color = fmt.Sprintf("Learn %s (tier %d) — %s", pk.Name, pk.Tier, pk.Desc), "#ffd27a"
		default:
			o.Label, o.Disabled = fmt.Sprintf("%s (tier %d, level %d) — %s", pk.Name, pk.Tier, perkLevel(pk.Tier), pk.Desc), true
		}
		d.optx(o)
	}
	g.show(p, d.opt("", "Back.").opt("close", "Close."))
}

// ---- spellbook

func (g *Game) spellMenu(p *Player, opt string) {
	c := p.Ch
	if rest, ok := strings.CutPrefix(opt, "slot:"); ok {
		id, n, _ := strings.Cut(rest, ":")
		k, err := strconv.Atoi(n)
		if err == nil && k >= 0 && k < len(c.SpellBar) && c.knows(id) {
			for i := range c.SpellBar {
				if c.SpellBar[i] == id {
					c.SpellBar[i] = ""
				}
			}
			c.SpellBar[k] = id
		}
	}
	if id, ok := strings.CutPrefix(opt, "bind:"); ok {
		sp := g.pack.Spell(id)
		if sp == nil || !c.knows(id) {
			return
		}
		d := menuDialog("spells", sp.Name, sp.Desc+"\n\nBind to which key?")
		for i, k := range spellKeys {
			cur := "empty"
			if s := g.pack.Spell(c.SpellBar[i]); s != nil {
				cur = s.Name
			}
			d.optx(protocol.Option{ID: fmt.Sprintf("slot:%s:%d", id, i), Label: fmt.Sprintf("[%s] currently: %s", k, cur)})
		}
		g.show(p, d.opt("", "Back."))
		return
	}
	d := menuDialog("spells", "Spellbook", fmt.Sprintf("Spell power %d · Mana %d/%d. Learn new spells from trainers at the Arcane Collegium. Click a spell to bind it.", int(p.S.SP), int(c.Mana), int(p.S.MaxMana)))
	d.Sub = "B"
	for _, id := range c.Spells {
		sp := g.pack.Spell(id)
		if sp == nil {
			continue
		}
		key := ""
		for i, s := range c.SpellBar {
			if s == id {
				key = "[" + spellKeys[i] + "] "
			}
		}
		dmg := sp.Power + p.S.SP*sp.Scale
		d.optx(protocol.Option{ID: "bind:" + id, Color: sp.Color, Label: fmt.Sprintf("%s%s — %.0f mana, %.1fs", key, sp.Name, sp.Mana, sp.CD*(1-p.S.CDR)),
			Hint: fmt.Sprintf("%s Power ≈ %.0f.", sp.Desc, dmg)})
	}
	g.show(p, d.opt("close", "Close."))
}

// ---- pets

func (g *Game) petMenu(p *Player, opt, text string) {
	c := p.Ch
	parts := strings.Split(opt, ":")
	idx := -1
	if len(parts) >= 2 {
		idx, _ = strconv.Atoi(parts[1])
	}
	valid := idx >= 0 && idx < len(c.Pets)
	switch parts[0] {
	case "summon":
		if valid {
			g.summonPet(p, idx)
		}
	case "dismiss":
		g.dismissPet(p)
	case "release":
		if valid {
			if c.ActivePet == idx {
				g.dismissPet(p)
			} else if c.ActivePet > idx {
				c.ActivePet--
				if p.Pet != nil {
					p.Pet.Index--
				}
			}
			p.system(c.Pets[idx].Name + " returns to the wild.")
			c.Pets = append(c.Pets[:idx], c.Pets[idx+1:]...)
		}
	case "rename":
		if valid {
			if name := sanitizeName(text); name != "" {
				c.Pets[idx].Name = name
			} else {
				d := menuDialog("pets|rename:"+parts[1], "Rename "+c.Pets[idx].Name, "Choose a new name.")
				d.Prompt = "New name"
				g.show(p, d.opt("", "Back."))
				return
			}
		}
	case "pet":
		if valid {
			pd := c.Pets[idx]
			hp, dmg := g.petStats(p, &pd)
			ab := g.pack.Ability(pd.Ability)
			abName := pd.Ability
			if ab != nil {
				abName = ab.Name
			}
			d := menuDialog("pets", pd.Name, fmt.Sprintf("Level %d · XP %d/%d\nHealth %d · Damage %d · Ability: %s", pd.Level, pd.XP, petXPNeeded(pd.Level), int(hp), int(dmg), abName))
			if c.ActivePet == idx {
				d.opt("dismiss", "Dismiss.")
			} else {
				d.opt(fmt.Sprintf("summon:%d", idx), "Summon.")
			}
			d.opt(fmt.Sprintf("rename:%d", idx), "Rename.")
			d.optx(protocol.Option{ID: fmt.Sprintf("release:%d", idx), Label: "Release into the wild.", Color: "#ff7766"})
			g.show(p, d.opt("", "Back."))
			return
		}
	}
	d := menuDialog("pets", "Pets", "Buy pets from stablemasters or tame weakened beasts with a Taming Rope (target the beast and press Y). Your active pet fights at your side and levels up with you.")
	d.Sub = "H"
	for i, pd := range c.Pets {
		state := ""
		if c.ActivePet == i {
			state = " (active)"
		}
		d.optx(protocol.Option{ID: fmt.Sprintf("pet:%d", i), Label: fmt.Sprintf("%s — level %d%s", pd.Name, pd.Level, state), Color: pd.Color})
	}
	if len(c.Pets) == 0 {
		d.Text += "\n\nYou have no pets yet."
	}
	g.show(p, d.opt("close", "Close."))
}

// ---- reputation

func (g *Game) repMenu(p *Player) {
	var lines []string
	fs := append([]string(nil), func() []string {
		ids := []string{}
		for _, f := range g.pack.Factions {
			ids = append(ids, f.ID)
		}
		return ids
	}()...)
	sort.SliceStable(fs, func(a, b int) bool { return g.rep(p, fs[a]) > g.rep(p, fs[b]) })
	for _, id := range fs {
		f := g.pack.Faction(id)
		r := g.repRank(p, id)
		ranks := g.pack.FactionsT.Ranks
		next := ""
		if i := g.repRankIndex(p, id); i+1 < len(ranks) {
			next = fmt.Sprintf(" → %s at %d", ranks[i+1].Name, ranks[i+1].Min)
		}
		lines = append(lines, fmt.Sprintf("[color=%s]%s[/color]: [color=%s]%s[/color] (%d%s)\n   %s", f.Color, f.Name, r.Color, r.Name, g.rep(p, id), next, f.Desc))
	}
	d := menuDialog("rep", "Reputation", strings.Join(lines, "\n"))
	d.Sub = "J"
	g.show(p, d.opt("close", "Close."))
}

// ---- contract journal

func (g *Game) journalMenu(p *Player, opt string) {
	c := p.Ch
	if id, ok := strings.CutPrefix(opt, "abandon:"); ok {
		cid, _ := strconv.ParseInt(id, 10, 64)
		for k, ct := range c.Contracts {
			if ct.ID == cid {
				c.Contracts = append(c.Contracts[:k], c.Contracts[k+1:]...)
				for eid, e := range g.enemies {
					if e.ContractID == cid {
						delete(g.enemies, eid)
					}
				}
				break
			}
		}
	}
	text := "Contracts are offered on boards in every town tavern."
	if len(c.Contracts) == 0 {
		text += "\n\nYou have no active contracts."
	}
	d := menuDialog("journal", "Contracts", text)
	d.Sub = "L"
	for _, ct := range c.Contracts {
		status := fmt.Sprintf("%d/%d", ct.Have, ct.Need)
		color := "#e0d0a0"
		if ct.Done {
			status, color = "complete — return to a board", "#8aff8a"
		}
		d.optx(protocol.Option{ID: fmt.Sprintf("abandon:%d", ct.ID), Label: fmt.Sprintf("%s [%s] (click to abandon)", ct.Title, status), Hint: ct.Desc, Color: color})
	}
	g.show(p, d.opt("close", "Close."))
}

// ---- appearance

func (g *Game) lookMenu(p *Player, opt string) {
	l := &p.Ch.Look
	cyc := func(v *int, n int) { *v = (*v + 1) % n }
	switch opt {
	case "skin":
		cyc(&l.Skin, len(skinNames))
	case "hair":
		cyc(&l.Hair, len(hairNames))
	case "hcolor":
		cyc(&l.HairColor, len(hairColors))
	case "beard":
		cyc(&l.Beard, len(beardNames))
	case "build":
		cyc(&l.Build, len(buildNames))
	case "eyes":
		cyc(&l.Eyes, len(eyeNames))
	case "model":
		cyc(&l.Model, len(modelNames))
	case "sex":
		l.Female = !l.Female
		if l.Female {
			l.Beard = 0
		}
	case "done":
		l.Done = true
		g.closeDialog(p)
		p.system("Looking sharp! Press K for your character sheet, B for spells, H for pets, J for reputation, L for contracts.")
		return
	}
	body := "Male"
	if l.Female {
		body = "Female"
	}
	d := menuDialog("look", "Appearance", "Shape your hero. Watch your character change as you click. A barber in Velhrad can change this later.")
	d.opt("model", "Hero: "+modelNames[l.Model%len(modelNames)])
	if l.Model%len(modelNames) == len(modelNames)-1 {
		d.opt("sex", "Body: "+body)
	}
	d.opt("skin", "Skin: "+skinNames[l.Skin%len(skinNames)])
	d.opt("hair", "Hair: "+hairNames[l.Hair%len(hairNames)])
	d.opt("hcolor", "Hair color: "+hairColors[l.HairColor%len(hairColors)])
	if !l.Female {
		d.opt("beard", "Beard: "+beardNames[l.Beard%len(beardNames)])
	}
	d.opt("build", "Build: "+buildNames[l.Build%len(buildNames)])
	d.opt("eyes", "Eyes: "+eyeNames[l.Eyes%len(eyeNames)])
	d.optx(protocol.Option{ID: "done", Label: "Done.", Color: "#8aff8a"})
	g.show(p, d)
}

var _ = math.Abs
