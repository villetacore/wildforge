package game

import (
	"fmt"
	"math"
	"strings"

	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/protocol"
)

// NPC is a static townsperson. Talking to one opens a server-driven dialog.
type NPC struct {
	ID      int64
	Role    *content.Role
	Name    string
	Faction string
	Town    int
	Tier    int
	X, Y    float64
	Yaw     float64
	Look    protocol.Look
}

const talkRange = 2.6

func (g *Game) spawnNPCs() {
	used := map[string]int{}
	for i, s := range g.world.NPCSpots {
		role := g.pack.Role(s.Role)
		if role == nil {
			continue
		}
		name := s.Name
		if name == "" {
			k := used[role.ID]
			used[role.ID]++
			name = role.Names[(k+s.Town)%len(role.Names)]
		}
		faction := role.Faction
		if faction == "" {
			faction = g.world.Towns[s.Town].Faction
		}
		h := uint32(i*2654435761) ^ uint32(len(name)*97)
		look := protocol.Look{Skin: int(h % 5), Hair: int(h>>3) % 6, HairColor: int(h>>7) % 7, Beard: int(h>>11) % 4, Build: int(h>>13) % 3, Done: true}
		if role.ID == "courtesan" || strings.HasPrefix(name, "Sister") || strings.HasPrefix(name, "Lady") || femaleName(name) {
			look.Female, look.Beard = true, 0
		}
		t := g.world.Towns[s.Town]
		n := &NPC{ID: g.newID(), Role: role, Name: name, Faction: faction, Town: s.Town, Tier: s.Tier, X: s.X, Y: s.Y, Look: look,
			Yaw: math.Atan2(t.X-s.X, t.Y-s.Y)}
		g.npcs[n.ID] = n
	}
}

func femaleName(n string) bool {
	for _, f := range []string{"Marta", "Jarka", "Lenka", "Greta", "Hilda", "Ilse", "Libuse", "Miroslava", "Agnes", "Dobra", "Hana", "Vlasta", "Lada", "Bara", "Petra", "Ivana", "Ludmila", "Anezka", "Bozena", "Ruzena", "Saba", "Zuzana"} {
		if strings.Contains(n, f) {
			return true
		}
	}
	return false
}

func (g *Game) npcViews() []protocol.NPC {
	out := make([]protocol.NPC, 0, len(g.npcs))
	for _, n := range g.npcs {
		out = append(out, protocol.NPC{
			ID: n.ID, Name: n.Name, Title: n.Role.Title, Role: n.Role.ID, Outfit: n.Role.Outfit, Color: n.Role.Color,
			Marker: n.Role.Marker, Faction: n.Faction, X: n.X, Y: n.Y, Yaw: n.Yaw, Look: n.Look,
		})
	}
	return out
}

func (g *Game) nearestNPC(p *Player, rng float64) *NPC {
	var best *NPC
	bd := rng
	for _, n := range g.npcs {
		if d := dist(p.Ch.X, p.Ch.Y, n.X, n.Y); d < bd {
			bd, best = d, n
		}
	}
	return best
}

// ---- dialog helpers

type dlg struct {
	protocol.Dialog
}

func newDialog(npc *NPC, menu, title, text string) *dlg {
	d := &dlg{protocol.Dialog{T: "dialog", Menu: menu, Title: title, Text: text, Options: []protocol.Option{}}}
	if npc != nil {
		d.NPC = npc.ID
		d.Sub = npc.Role.Title
	}
	return d
}

func (d *dlg) opt(id, label string) *dlg {
	d.Options = append(d.Options, protocol.Option{ID: id, Label: label})
	return d
}

func (d *dlg) optx(o protocol.Option) *dlg {
	d.Options = append(d.Options, o)
	return d
}

func (d *dlg) bye() *dlg { return d.opt("bye", "Goodbye.") }

func (g *Game) show(p *Player, d *dlg) {
	p.Dialog = d.NPC
	p.send(d.Dialog)
}

func (g *Game) closeDialog(p *Player) {
	p.Dialog = 0
	p.send(protocol.DialogClose{T: "dialog_close"})
}

// price applies faction reputation and the Haggler perk to a base price.
func (g *Game) price(p *Player, npc *NPC, base int, selling bool) int {
	mult := 1.0
	if npc != nil {
		mult = g.repRank(p, npc.Faction).Price
	}
	if selling {
		v := float64(base) * 0.3 * (1 + p.S.Price)
		return max(1, int(math.Round(v)))
	}
	return max(1, int(math.Ceil(float64(base)*mult*(1-p.S.Price))))
}

func (g *Game) talkTo(p *Player, n *NPC) {
	if rr := g.repRank(p, n.Faction); rr.Price == 0 && n.Role.ID != "board" {
		g.show(p, newDialog(n, "", n.Name, fmt.Sprintf("%s glares at you. \"The %s doesn't deal with your kind. Get lost.\"", n.Name, g.factionName(n.Faction))).bye())
		return
	}
	g.npcMenu(p, n, "")
}

// talk handles a dialog choice.
func (g *Game) talk(p *Player, m protocol.ClientMsg) {
	if m.Opt == "bye" || m.Opt == "close" {
		g.closeDialog(p)
		return
	}
	if m.NPC == 0 {
		base, sub, hasSub := strings.Cut(m.Menu, "|")
		if m.Opt == "submit" && hasSub {
			g.menuChoice(p, base, sub, m.Text)
		} else {
			g.menuChoice(p, base, m.Opt, "")
		}
		return
	}
	n, ok := g.npcs[m.NPC]
	if !ok {
		return
	}
	if dist(p.Ch.X, p.Ch.Y, n.X, n.Y) > talkRange+1.5 {
		g.closeDialog(p)
		p.system("You walked away from " + n.Name + ".")
		return
	}
	if m.Opt == "submit" {
		g.npcMenu(p, n, m.Menu+"|"+m.Text)
		return
	}
	g.npcMenu(p, n, m.Opt)
}

// npcMenu routes a choice (possibly "" for the greeting) to the role handler.
func (g *Game) npcMenu(p *Player, n *NPC, opt string) {
	switch {
	case opt == "":
		g.greet(p, n)
	case opt == "shop" || strings.HasPrefix(opt, "buy:"):
		g.shop(p, n, opt)
	case opt == "sell" || strings.HasPrefix(opt, "sell:"):
		g.sellMenu(p, n, opt)
	case opt == "rumor":
		r := g.pack.NPCs.Rumors
		g.show(p, newDialog(n, "", n.Name, "\""+r[int(g.clock)%len(r)]+"\"").opt("", "Tell me more.").bye())
	default:
		switch n.Role.ID {
		case "innkeeper":
			g.innkeeper(p, n, opt)
		case "board":
			g.contractBoard(p, n, opt)
		case "courtesan":
			g.courtesan(p, n, opt)
		case "blacksmith":
			g.forge(p, n, opt)
		case "enchanter":
			g.enchanter(p, n, opt)
		case "trainer":
			g.trainer(p, n, opt)
		case "healer":
			g.healer(p, n, opt)
		case "barber":
			g.menuChoice(p, "look", opt, "")
		case "stablemaster", "exotic":
			g.stable(p, n, opt)
		case "quartermaster":
			g.quartermaster(p, n, opt)
		case "king":
			g.king(p, n, opt)
		default:
			g.greet(p, n)
		}
	}
}

func (g *Game) greet(p *Player, n *NPC) {
	r := n.Role
	text := r.Greeting
	d := newDialog(n, "", n.Name, text)
	if len(r.Shop) > 0 {
		d.opt("shop", "Show me your wares.")
		d.opt("sell", "I have things to sell.")
	}
	switch r.ID {
	case "innkeeper":
		d.opt("room", fmt.Sprintf("Rent a room for the night (%dg).", g.price(p, n, 25, false)))
		d.opt("rumor", "Heard any rumors?")
		d.opt("contracts", "Any work for a sword-arm?")
	case "board":
		g.contractBoard(p, n, "")
		return
	case "courtesan":
		g.courtesan(p, n, "")
		return
	case "blacksmith":
		d.opt("forge", "Forge me something custom.")
		d.opt("engrave", "Engrave a name on my gear.")
		d.opt("salvage", "Salvage gear for materials.")
	case "enchanter":
		d.opt("enchant", "Enchant an item.")
		d.opt("disenchant", "Disenchant an item into arcane dust.")
	case "trainer":
		g.trainer(p, n, "")
		return
	case "healer":
		d.opt("cure", fmt.Sprintf("Cure my diseases (%dg).", g.price(p, n, 40, false)))
		d.opt("heal", "Heal my wounds (free).")
	case "barber":
		d.opt("look", "Change my appearance.")
	case "stablemaster", "exotic":
		d.opt("pets", "About my pets...")
	case "quartermaster":
		g.quartermaster(p, n, "")
		return
	case "king":
		g.king(p, n, "")
		return
	case "guard":
		d.Text = r.Greeting + "\n\n" + g.repLine(p, n.Faction)
	}
	g.show(p, d.bye())
}

// shop lists items for sale or buys one.
func (g *Game) shop(p *Player, n *NPC, opt string) {
	if id, ok := strings.CutPrefix(opt, "buy:"); ok {
		allowed := false
		for _, s := range n.Role.Shop {
			if s == id {
				allowed = true
			}
		}
		def := g.pack.Item(id)
		if !allowed || def == nil {
			return
		}
		cost := g.price(p, n, def.Price, false)
		switch {
		case p.Ch.Gold < cost:
			p.system("You can't afford " + def.Name + ".")
		case p.Ch.inv(g.pack).Add(id, 1) > 0:
			p.system("Inventory full.")
		default:
			p.Ch.Gold -= cost
			g.addFx("gold", p.Ch.X, p.Ch.Y, fmt.Sprintf("-%dg", cost))
			g.changeRep(p, n.Faction, 2)
		}
	}
	d := newDialog(n, "", n.Name, fmt.Sprintf("You have %d gold.", p.Ch.Gold))
	for _, id := range n.Role.Shop {
		def := g.pack.Item(id)
		cost := g.price(p, n, def.Price, false)
		d.optx(protocol.Option{ID: "buy:" + id, Label: fmt.Sprintf("%s — %dg", def.Name, cost), Hint: def.Desc, Color: def.Color, Disabled: p.Ch.Gold < cost})
	}
	g.show(p, d.opt("", "Back.").bye())
}

func (g *Game) sellMenu(p *Player, n *NPC, opt string) {
	if s, ok := strings.CutPrefix(opt, "sell:"); ok {
		var idx int
		if _, err := fmt.Sscan(s, &idx); err == nil && idx >= 0 && idx < len(p.Ch.Inventory) {
			st, _ := p.Ch.inv(g.pack).TakeAt(idx, 1)
			gold := g.price(p, n, g.stackValue(&st), true)
			p.Ch.Gold += gold
			g.addFx("gold", p.Ch.X, p.Ch.Y, fmt.Sprintf("+%dg", gold))
		}
	}
	d := newDialog(n, "", n.Name, fmt.Sprintf("What are you selling? You have %d gold.", p.Ch.Gold))
	for i, s := range p.Ch.Inventory {
		v := g.price(p, n, g.stackValue(&s), true)
		label := fmt.Sprintf("%s ×%d — %dg each", g.stackName(&s), s.Qty, v)
		if s.Qty == 1 {
			label = fmt.Sprintf("%s — %dg", g.stackName(&s), v)
		}
		o := protocol.Option{ID: fmt.Sprintf("sell:%d", i), Label: label}
		if s.G != nil {
			o.Color = s.G.Color
		}
		d.optx(o)
	}
	g.show(p, d.opt("", "Back.").bye())
}
