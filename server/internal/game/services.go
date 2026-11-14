package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"sort"
	"strconv"
	"strings"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

// ---- tavern

func (g *Game) innkeeper(p *Player, n *NPC, opt string) {
	switch opt {
	case "room":
		cost := g.price(p, n, 25, false)
		if p.Ch.Gold < cost {
			g.show(p, newDialog(n, "", n.Name, "\"No coin, no bed. The stable's free if you don't mind the horses.\"").bye())
			return
		}
		p.Ch.Gold -= cost
		p.Ch.HP, p.Ch.Mana = p.S.MaxHP, p.S.MaxMana
		p.Ch.Mood = math.Min(100, p.Ch.Mood+15)
		g.addStatus(&p.Ch.Statuses, "well_rested", 1)
		removeStatuses(&p.Ch.Statuses, func(s Status) bool { d := g.pack.Status(s.ID); return d == nil || !d.Bad || d.Disease })
		g.addFx("heal", p.Ch.X, p.Ch.Y, "Well Rested")
		g.show(p, newDialog(n, "", n.Name, "You sleep like a log on a straw mattress that smells faintly of ale. You wake up refreshed.\n\n[color=#ffd27a]Well Rested: +25% experience for 30 minutes.[/color]").bye())
	case "contracts":
		g.contractBoard(p, n, "")
	default:
		g.greet(p, n)
	}
}

type courtesanTier struct {
	price   int
	mood    float64
	disease float64
	status  string
	text    string
}

var courtesanTiers = []courtesanTier{
	{15, 20, 0.35, "", "The room is cramped, the sheets are... used. She is cheerful and quick, and you leave lighter in the purse and strangely itchy in spirit."},
	{60, 45, 0.08, "", "A clean room, a bottle of wine and good conversation by candlelight. You leave in excellent spirits."},
	{250, 80, 0, "inspired", "Silk, music, poetry and a breakfast served on silver. You leave feeling like the hero of a ballad."},
}

func (g *Game) courtesan(p *Player, n *NPC, opt string) {
	t := courtesanTiers[min(n.Tier, len(courtesanTiers)-1)]
	price := g.price(p, n, t.price, false)
	risk := t.disease
	if p.S.Charm {
		price = int(float64(price) * 0.7)
		risk *= 0.3
	}
	risk *= 1 - p.S.DiseaseResist
	if opt == "night" {
		if p.Ch.Gold < price {
			g.show(p, newDialog(n, "", n.Name, "\"Come back when your purse is as eager as you are, darling.\"").bye())
			return
		}
		p.Ch.Gold -= price
		before := p.Ch.Mood
		p.Ch.Mood = math.Min(100, p.Ch.Mood+t.mood)
		p.Ch.HP, p.Ch.Mana = p.S.MaxHP, p.S.MaxMana
		text := t.text + fmt.Sprintf("\n\n[color=#ff9ad0]Mood %d → %d[/color]", int(before), int(p.Ch.Mood))
		if t.status != "" {
			g.addStatus(&p.Ch.Statuses, t.status, 1)
			text += "\n[color=#ff9ad0]Inspired: +10% experience, +3% critical chance.[/color]"
		}
		if rand.Float64() < risk {
			diseases := []string{"disease_gonorrhea", "disease_chlamydia", "disease_lice"}
			id := diseases[rand.IntN(len(diseases))]
			g.addStatus(&p.Ch.Statuses, id, 1)
			d := g.pack.Status(id)
			text += fmt.Sprintf("\n\n[color=#c0c060]A few days later something burns, itches and generally feels wrong. You have contracted %s. %s[/color]", d.Name, d.Desc)
		}
		g.addFx("heal", p.Ch.X, p.Ch.Y, "♥")
		g.show(p, newDialog(n, "", n.Name, text).bye())
		return
	}
	pitch := []string{
		"\"Fifteen silver— er, gold, and I'm yours till dawn. No refunds, no questions.\"",
		"\"Sixty gold buys you a clean room, a good wine and my undivided attention.\"",
		"\"My company is not bought, sweetling, it is earned. Though two hundred and fifty gold does help.\"",
	}[min(n.Tier, 2)]
	risky := []string{"[color=#c0c060]Rumor has it her customers often visit the healer afterwards.[/color]", "[color=#8ac08a]Discreet and clean, mostly.[/color]", "[color=#8ad0ff]The finest company in the realm. Perfectly safe.[/color]"}[min(n.Tier, 2)]
	d := newDialog(n, "", n.Name, pitch+"\n\n"+risky+fmt.Sprintf("\nYour mood: %d / 100.", int(p.Ch.Mood)))
	d.optx(protocol.Option{ID: "night", Label: fmt.Sprintf("Spend the night (%dg).", price), Disabled: p.Ch.Gold < price, Color: "#ff9ad0"})
	g.show(p, d.bye())
}

// ---- healer

func (g *Game) healer(p *Player, n *NPC, opt string) {
	switch opt {
	case "cure":
		cost := g.price(p, n, 40, false)
		diseased := false
		for _, s := range p.Ch.Statuses {
			if d := g.pack.Status(s.ID); d != nil && d.Disease {
				diseased = true
			}
		}
		if !diseased {
			g.show(p, newDialog(n, "", n.Name, "\"You're perfectly healthy. Whatever you're worried about, it's in your head.\"").bye())
			return
		}
		if p.Ch.Gold < cost {
			g.show(p, newDialog(n, "", n.Name, "\"The Gods heal for free. Medicine costs money.\"").bye())
			return
		}
		p.Ch.Gold -= cost
		removeStatuses(&p.Ch.Statuses, func(s Status) bool { d := g.pack.Status(s.ID); return d == nil || !d.Disease })
		g.addFx("heal", p.Ch.X, p.Ch.Y, "Cured")
		g.show(p, newDialog(n, "", n.Name, "\"Drink this. All of it. Yes, it tastes like that. And maybe be pickier about your company.\"\n\n[color=#8aff8a]All diseases cured.[/color]").bye())
	case "heal":
		p.Ch.HP = p.S.MaxHP
		removeStatuses(&p.Ch.Statuses, func(s Status) bool { d := g.pack.Status(s.ID); return d == nil || !d.Bad || d.Disease })
		g.addFx("heal", p.Ch.X, p.Ch.Y, "Healed")
		g.greet(p, n)
	default:
		g.greet(p, n)
	}
}

// ---- spell trainer

func (g *Game) trainer(p *Player, n *NPC, opt string) {
	msg := "\"Which words of power would you learn?\""
	if id, ok := strings.CutPrefix(opt, "learn:"); ok {
		if sp := g.pack.Spell(id); sp != nil {
			cost := g.price(p, n, sp.Price, false)
			switch {
			case p.Ch.Gold < cost:
				msg = "\"Knowledge is priceless. This spell, however, costs " + strconv.Itoa(cost) + " gold.\""
			case g.learnSpell(p, id):
				p.Ch.Gold -= cost
				g.changeRep(p, "collegium", 50)
				msg = "\"Well done. Now practise — away from the library, please.\""
			}
		}
	}
	d := newDialog(n, "", n.Name, msg)
	spells := append([]content.Spell(nil), g.pack.Magic.Spells...)
	sort.Slice(spells, func(a, b int) bool { return spells[a].Level < spells[b].Level })
	for _, sp := range spells {
		if p.Ch.knows(sp.ID) {
			continue
		}
		cost := g.price(p, n, sp.Price, false)
		d.optx(protocol.Option{
			ID: "learn:" + sp.ID, Color: sp.Color,
			Label:    fmt.Sprintf("%s (level %d) — %dg", sp.Name, sp.Level, cost),
			Hint:     fmt.Sprintf("%s %s · %.0f mana · %.1fs cooldown", sp.Desc, strings.ToUpper(sp.School[:1])+sp.School[1:], sp.Mana, sp.CD),
			Disabled: p.Ch.Level < sp.Level || p.Ch.Gold < cost,
		})
	}
	g.show(p, d.bye())
}

// ---- blacksmith: forging custom gear, salvage, engraving

var forgeTiers = []struct {
	name            string
	fee             float64
	quality, rarity float64
}{
	{"Standard work", 1, 0.3, 0},
	{"Careful work", 2.2, 1.2, 0.6},
	{"Masterwork attempt", 5, 2.5, 1.6},
}

func (g *Game) forgeFee(p *Player, n *NPC, b *content.GearBase, m *content.Material, tier int) int {
	base := 10 + float64(m.Price*b.Qty)*0.6 + float64(p.Ch.Level)*3
	return g.price(p, n, int(base*forgeTiers[tier].fee), false)
}

func (g *Game) forge(p *Player, n *NPC, opt string) {
	parts := strings.Split(opt, ":")
	if rest, ok := strings.CutPrefix(opt, "engrave:"); ok {
		parts = []string{"engrave", rest}
	}
	switch parts[0] {
	case "forge":
		switch len(parts) {
		case 1:
			d := newDialog(n, "", n.Name, "\"What shall it be? Bring the material, I bring the skill.\"")
			for _, b := range g.pack.Gear.Bases {
				d.optx(protocol.Option{ID: "forge:" + b.ID, Label: fmt.Sprintf("%s (%s, %s)", b.Name, strings.ToLower(slotName(b.Slot)), b.Mats), Disabled: b.Level > p.Ch.Level+2,
					Hint: fmt.Sprintf("Requires level %d", b.Level)})
			}
			g.show(p, d.opt("", "Back.").bye())
		case 2:
			b := g.pack.Base(parts[1])
			if b == nil {
				return
			}
			d := newDialog(n, "", n.Name, fmt.Sprintf("\"A %s. Which material?\"", strings.ToLower(b.Name)))
			for _, m := range g.pack.Gear.Materials {
				if m.Kind != b.Mats {
					continue
				}
				have := p.Ch.inv(g.pack).Count(m.Item)
				d.optx(protocol.Option{ID: "forge:" + b.ID + ":" + m.ID, Color: m.Color,
					Label:    fmt.Sprintf("%s — %d× %s (you have %d)", m.Name, b.Qty, g.pack.Item(m.Item).Name, have),
					Disabled: have < b.Qty || m.Level > p.Ch.Level+3, Hint: fmt.Sprintf("Material tier %.2f, requires level %d", m.Tier, m.Level)})
			}
			g.show(p, d.opt("forge", "Back.").bye())
		case 3:
			b, m := g.pack.Base(parts[1]), g.pack.Material(parts[2])
			if b == nil || m == nil {
				return
			}
			d := newDialog(n, "", n.Name, "\"How much care should I put into it? Better work costs more but gives finer quality and a chance of magic.\"")
			for i, t := range forgeTiers {
				fee := g.forgeFee(p, n, b, m, i)
				d.optx(protocol.Option{ID: fmt.Sprintf("forge:%s:%s:%d", b.ID, m.ID, i), Label: fmt.Sprintf("%s — %dg", t.name, fee), Disabled: p.Ch.Gold < fee})
			}
			g.show(p, d.opt("forge:"+b.ID, "Back.").bye())
		case 4:
			b, m := g.pack.Base(parts[1]), g.pack.Material(parts[2])
			tier, _ := strconv.Atoi(parts[3])
			if b == nil || m == nil || tier < 0 || tier >= len(forgeTiers) {
				return
			}
			fee := g.forgeFee(p, n, b, m, tier)
			inv := p.Ch.inv(g.pack)
			if p.Ch.Gold < fee || inv.Count(m.Item) < b.Qty || m.Kind != b.Mats {
				p.system("You lack the gold or materials.")
				return
			}
			if inv.Full() && inv.Count(m.Item) > b.Qty {
				p.system("Inventory full.")
				return
			}
			inv.Remove(m.Item, b.Qty)
			p.Ch.Gold -= fee
			t := forgeTiers[tier]
			gear := g.genGear(p.Ch.Level, GearSpec{Base: b.ID, Mat: m.ID, Quality: -1, Rarity: -1, QualityBoost: t.quality, RarityBoost: t.rarity, Crafter: p.Name})
			inv.AddGear(gear)
			idx := len(p.Ch.Inventory) - 1
			g.changeRep(p, n.Faction, 15)
			g.addFx("loot", p.Ch.X, p.Ch.Y, "Forged "+gear.Name)
			d := newDialog(n, "", n.Name, fmt.Sprintf("Sparks fly, steel sings. The smith hands you:\n\n[color=%s]%s[/color]\n%s", gear.Color, gear.Name, strings.Join(gear.Lines, "\n")))
			d.opt(fmt.Sprintf("engrave:%d", idx), "Engrave a name on it.")
			d.opt("forge", "Forge another.")
			g.show(p, d.bye())
		}
	case "engrave":
		if len(parts) == 1 {
			d := newDialog(n, "", n.Name, "\"Which piece should bear your words?\"")
			for i, s := range p.Ch.Inventory {
				if s.G != nil {
					d.optx(protocol.Option{ID: fmt.Sprintf("engrave:%d", i), Label: s.G.Name, Color: s.G.Color})
				}
			}
			g.show(p, d.opt("", "Back.").bye())
			return
		}
		idxText, name, hasName := strings.Cut(parts[1], "|")
		idx, err := strconv.Atoi(idxText)
		if err != nil || idx < 0 || idx >= len(p.Ch.Inventory) || p.Ch.Inventory[idx].G == nil {
			return
		}
		gear := p.Ch.Inventory[idx].G
		if !hasName {
			d := newDialog(n, "", n.Name, "\"What name shall I engrave on your "+gear.Name+"? (5 gold)\"")
			d.Menu = "engrave:" + idxText
			d.Prompt = "Name (up to 24 letters)"
			g.show(p, d.opt("", "Never mind."))
			return
		}
		name = sanitizeName(name)
		if name == "" || p.Ch.Gold < 5 {
			g.greet(p, n)
			return
		}
		p.Ch.Gold -= 5
		gear.Custom = name
		g.refreshGear(gear)
		g.show(p, newDialog(n, "", n.Name, fmt.Sprintf("\"There. [color=%s]%s[/color]. Wear it with pride.\"", gear.Color, gear.Name)).bye())
	case "salvage":
		if len(parts) == 2 {
			idx, err := strconv.Atoi(parts[1])
			if err == nil && idx >= 0 && idx < len(p.Ch.Inventory) && p.Ch.Inventory[idx].G != nil {
				gear := p.Ch.Inventory[idx].G
				b, m := g.pack.Base(gear.Base), g.pack.Material(gear.Mat)
				p.Ch.inv(g.pack).TakeAt(idx, 1)
				if b != nil && m != nil {
					p.Ch.inv(g.pack).Add(m.Item, max(1, b.Qty/2))
					p.system(fmt.Sprintf("Salvaged %d× %s.", max(1, b.Qty/2), g.pack.Item(m.Item).Name))
				}
				if gear.Rarity > 0 {
					p.Ch.inv(g.pack).Add("arcane_dust", gear.Rarity)
				}
			}
		}
		d := newDialog(n, "", n.Name, "\"I'll melt it down and give you back what's usable.\"")
		for i, s := range p.Ch.Inventory {
			if s.G != nil {
				d.optx(protocol.Option{ID: fmt.Sprintf("salvage:%d", i), Label: s.G.Name, Color: s.G.Color})
			}
		}
		g.show(p, d.opt("", "Back.").bye())
	default:
		g.greet(p, n)
	}
}

func sanitizeName(s string) string {
	s = strings.TrimSpace(s)
	var b strings.Builder
	for _, r := range s {
		if b.Len() >= 24 {
			break
		}
		if r == ' ' || r == '\'' || r == '-' || (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r > 127 {
			b.WriteRune(r)
		}
	}
	return strings.TrimSpace(b.String())
}

// ---- enchanter

// gearRef resolves "e.<slot>" (equipped) or "i.<index>" (inventory).
func (g *Game) gearRef(p *Player, ref string) *content.Gear {
	if slot, ok := strings.CutPrefix(ref, "e."); ok {
		if s := p.Ch.Equip[slot]; s != nil {
			return s.G
		}
		return nil
	}
	if i, ok := strings.CutPrefix(ref, "i."); ok {
		idx, err := strconv.Atoi(i)
		if err == nil && idx >= 0 && idx < len(p.Ch.Inventory) {
			return p.Ch.Inventory[idx].G
		}
	}
	return nil
}

func (g *Game) gearList(p *Player, d *dlg, prefix string) {
	for _, slot := range slots {
		if s := p.Ch.Equip[slot]; s != nil && s.G != nil {
			d.optx(protocol.Option{ID: prefix + "e." + slot, Label: "[equipped] " + s.G.Name, Color: s.G.Color, Hint: strings.Join(s.G.Lines, "\n")})
		}
	}
	for i, s := range p.Ch.Inventory {
		if s.G != nil {
			d.optx(protocol.Option{ID: fmt.Sprintf("%si.%d", prefix, i), Label: s.G.Name, Color: s.G.Color, Hint: strings.Join(s.G.Lines, "\n")})
		}
	}
}

func (g *Game) enchanter(p *Player, n *NPC, opt string) {
	parts := strings.Split(opt, ":")
	switch parts[0] {
	case "enchant":
		if len(parts) == 1 {
			d := newDialog(n, "", n.Name, "\"Choose the item to enchant. A new enchantment replaces the old one.\"")
			g.gearList(p, d, "enchant:")
			g.show(p, d.opt("", "Back.").bye())
			return
		}
		gear := g.gearRef(p, parts[1])
		if gear == nil {
			return
		}
		if len(parts) == 3 {
			e := g.pack.EnchantDef(parts[2])
			if e == nil || !slotAllowed(e.Slots, gear.Slot) {
				return
			}
			cost := g.price(p, n, e.Gold, false)
			inv := p.Ch.inv(g.pack)
			for id, q := range e.Cost {
				if inv.Count(id) < q {
					p.system("Missing reagents.")
					return
				}
			}
			if p.Ch.Gold < cost {
				p.system("Not enough gold.")
				return
			}
			for id, q := range e.Cost {
				inv.Remove(id, q)
			}
			p.Ch.Gold -= cost
			gear = g.gearRef(p, parts[1]) // inventory may have shifted
			if gear == nil {
				return
			}
			gear.Enchant = e.ID
			g.refreshGear(gear)
			g.changeRep(p, n.Faction, 20)
			g.addFx("level", p.Ch.X, p.Ch.Y, e.Name+"!")
			g.show(p, newDialog(n, "", n.Name, fmt.Sprintf("Runes flare and fade into the metal.\n\n[color=%s]%s[/color]\n%s", gear.Color, gear.Name, strings.Join(gear.Lines, "\n"))).opt("enchant", "Enchant another.").bye())
			return
		}
		d := newDialog(n, "", n.Name, fmt.Sprintf("[color=%s]%s[/color]\n%s", gear.Color, gear.Name, strings.Join(gear.Lines, "\n")))
		inv := p.Ch.inv(g.pack)
		for _, e := range g.pack.Gear.Enchants {
			if !slotAllowed(e.Slots, gear.Slot) {
				continue
			}
			cost := g.price(p, n, e.Gold, false)
			var reag []string
			ok := p.Ch.Gold >= cost && gear.Level >= e.Level
			for id, q := range e.Cost {
				reag = append(reag, fmt.Sprintf("%d %s (%d)", q, g.pack.Item(id).Name, inv.Count(id)))
				ok = ok && inv.Count(id) >= q
			}
			sort.Strings(reag)
			var st []string
			for k, v := range e.Stats {
				st = append(st, fmtStat(k, v))
			}
			sort.Strings(st)
			d.optx(protocol.Option{ID: "enchant:" + parts[1] + ":" + e.ID, Label: fmt.Sprintf("%s (%s) — %dg", e.Name, strings.Join(st, ", "), cost),
				Hint: "Needs " + strings.Join(reag, ", ") + fmt.Sprintf(". Item level %d+.", e.Level), Disabled: !ok})
		}
		g.show(p, d.opt("enchant", "Back.").bye())
	case "disenchant":
		if len(parts) == 2 {
			if gear := g.gearRef(p, parts[1]); gear != nil && strings.HasPrefix(parts[1], "i.") {
				idx, _ := strconv.Atoi(parts[1][2:])
				p.Ch.inv(g.pack).TakeAt(idx, 1)
				dust := 1 + gear.Rarity*2 + gear.Level/5
				p.Ch.inv(g.pack).Add("arcane_dust", dust)
				p.system(fmt.Sprintf("%s dissolves into %d arcane dust.", gear.Name, dust))
			}
		}
		d := newDialog(n, "", n.Name, "\"Unwanted magic? I'll unravel it into dust you can reuse.\"")
		for i, s := range p.Ch.Inventory {
			if s.G != nil {
				d.optx(protocol.Option{ID: fmt.Sprintf("disenchant:i.%d", i), Label: s.G.Name, Color: s.G.Color})
			}
		}
		g.show(p, d.opt("", "Back.").bye())
	default:
		g.greet(p, n)
	}
}

// ---- stable

func (g *Game) stable(p *Player, n *NPC, opt string) {
	if opt == "pets" {
		g.menuChoice(p, "pets", "", "")
		return
	}
	g.greet(p, n)
}

// ---- faction quartermaster

func (g *Game) quartermaster(p *Player, n *NPC, opt string) {
	rr := g.repRank(p, n.Faction)
	rank := g.repRankIndex(p, n.Faction)
	offers := []struct {
		rank   int
		name   string
		rarity int
		gold   int
	}{{4, "Requisition: fine gear", 2, 150}, {5, "Requisition: honored gear", 3, 400}, {6, "Requisition: revered relic", 4, 1200}}
	msg := fmt.Sprintf("\"The %s rewards its friends.\"\n\n%s", g.factionName(n.Faction), g.repLine(p, n.Faction))
	if strings.HasPrefix(opt, "req:") {
		i, _ := strconv.Atoi(opt[4:])
		if i >= 0 && i < len(offers) && rank >= offers[i].rank {
			o := offers[i]
			cost := o.gold + p.Ch.Level*20
			if p.Ch.Gold >= cost && !p.Ch.inv(g.pack).Full() {
				p.Ch.Gold -= cost
				gear := g.genGear(p.Ch.Level+1, GearSpec{Quality: 3, Rarity: o.rarity})
				p.Ch.inv(g.pack).AddGear(gear)
				msg = fmt.Sprintf("\"Take it, with our thanks.\"\n\n[color=%s]%s[/color]\n%s", gear.Color, gear.Name, strings.Join(gear.Lines, "\n"))
			} else {
				msg = "\"You lack the gold or the space for it.\""
			}
		}
	}
	d := newDialog(n, "", n.Name, msg)
	for i, o := range offers {
		cost := o.gold + p.Ch.Level*20
		d.optx(protocol.Option{ID: fmt.Sprintf("req:%d", i), Label: fmt.Sprintf("%s — %dg (%s)", o.name, cost, g.pack.FactionsT.Ranks[o.rank].Name),
			Disabled: rank < o.rank || p.Ch.Gold < cost, Color: g.pack.Gear.Rarities[o.rarity].Color})
	}
	_ = rr
	g.show(p, d.bye())
}

// ---- the king

func (g *Game) king(p *Player, n *NPC, opt string) {
	rank := g.repRankIndex(p, "crown")
	text := "\"Bandits on the roads, cultists in the east, something stirring beneath the cathedral... My realm needs heroes, not courtiers.\"\n\n" + g.repLine(p, "crown")
	d := newDialog(n, "", n.Name, text)
	knighted := strings.HasPrefix(p.Ch.Title, "Knight")
	if opt == "knight" && !knighted && rank >= 5 && p.Ch.Level >= 10 {
		p.Ch.Title = "Knight of Velhrad"
		gear := g.genGear(p.Ch.Level+2, GearSpec{Base: "longsword", Mat: "silversteel", Quality: 4, Rarity: 3})
		gear.Custom = "Kingsword"
		g.refreshGear(gear)
		p.Ch.inv(g.pack).AddGear(gear)
		g.broadcastSystem(fmt.Sprintf("King Radomir has knighted %s! Long live the Knight of Velhrad!", p.Name))
		d.Text = "\"Kneel.\" The flat of the royal blade touches each shoulder. \"Rise, Knight of Velhrad.\"\n\nYou received the [color=#b05aff]Kingsword[/color]."
		g.show(p, d.bye())
		return
	}
	if !knighted {
		d.optx(protocol.Option{ID: "knight", Label: "I wish to serve the Crown. (Knighthood: Honored with the Crown, level 10)", Disabled: rank < 5 || p.Ch.Level < 10})
	}
	d.opt("rumor", "What troubles the realm?")
	g.show(p, d.bye())
}
