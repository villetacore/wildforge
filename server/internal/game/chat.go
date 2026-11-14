package game

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode"

	"github.com/villetacore/wildforge/server/internal/protocol"
)

func (g *Game) chat(p *Player, text string) {
	text = strings.Map(func(r rune) rune {
		if unicode.IsControl(r) {
			return -1
		}
		return r
	}, strings.TrimSpace(text))
	if r := []rune(text); len(r) > 200 {
		text = string(r[:200])
	}
	switch text {
	case "":
		return
	case "/who":
		names := make([]string, 0, len(g.players))
		for _, o := range g.players {
			names = append(names, fmt.Sprintf("%s (lv %d)", o.Name, o.Ch.Level))
		}
		p.system(fmt.Sprintf("Online (%d): %s", len(names), strings.Join(names, ", ")))
		return
	case "/stats":
		c := p.Ch
		p.system(fmt.Sprintf("Level %d | XP %d/%d | ATK %d | DEF %d | SP %d | Gold %d | Kills %d | Bosses %d",
			c.Level, c.XP, XPForNext(c.Level), int(p.S.Atk), int(p.S.Def), int(p.S.SP), c.Gold, c.Kills, c.Bosses))
		return
	case "/help":
		p.system("WASD move (camera-relative), right-drag rotate camera, wheel zoom, left-click target/talk, Space attack, R F G T Z X spells, E interact, Y tame, Q potion, 1-9 items, C craft, K character, B spells, H pets, J reputation, L contracts, I inventory, M map, Enter chat. Commands: /who /stats /help /stuck")
		return
	case "/stuck":
		p.Ch.X, p.Ch.Y = g.world.SpawnX, g.world.SpawnY
		p.system("You were returned to Velhrad.")
		return
	}
	if strings.HasPrefix(text, "/") && g.devCommand(p, text) {
		return
	}
	g.broadcast(protocol.Chat{T: "chat", From: p.Name, Text: text})
}

// devCommand handles cheats for testing, enabled with WILDFORGE_DEV=1:
// /tp x y, /tpto <town>, /level n, /gold n, /give item [qty], /loot level,
// /pet id, /time 0..1, /spawn family [rank].
func (g *Game) devCommand(p *Player, text string) bool {
	if os.Getenv("WILDFORGE_DEV") != "1" {
		return false
	}
	f := strings.Fields(text)
	num := func(i int) float64 {
		if i < len(f) {
			v, _ := strconv.ParseFloat(f[i], 64)
			return v
		}
		return 0
	}
	c := p.Ch
	switch f[0] {
	case "/tp":
		c.X, c.Y = num(1), num(2)
	case "/tpto":
		for _, t := range g.world.Towns {
			if len(f) > 1 && strings.EqualFold(t.Name, f[1]) {
				c.X, c.Y = t.X+2.5, t.Y+2.5
			}
		}
	case "/level":
		c.Level = max(1, int(num(1)))
		c.StatPoints, c.PerkPoints = 2*(c.Level-1), c.Level-1
	case "/gold":
		c.Gold += int(num(1))
	case "/give":
		q := max(1, int(num(2)))
		if len(f) > 1 {
			c.inv(g.pack).Add(f[1], q)
		}
	case "/loot":
		for range 5 {
			c.inv(g.pack).AddGear(g.genGear(max(1, int(num(1))), GearSpec{Quality: -1, Rarity: -1, RarityBoost: 1.5}))
		}
	case "/pet":
		if len(f) > 1 {
			if d := g.pack.PetDef(f[1]); d != nil {
				g.adoptPet(p, newPetFromDef(d))
			}
		}
	case "/dungeon":
		if i := int(num(1)); i >= 0 && i < len(g.world.Dungeons) {
			l := g.world.Portals[2*i]
			c.X, c.Y = l.ToX, l.ToY+1
		}
	case "/npc":
		for _, n := range g.npcs {
			if len(f) > 1 && n.Role.ID == f[1] && n.Town == 0 {
				c.X, c.Y = n.X, n.Y+1.2
				break
			}
		}
	case "/god":
		p.God = !p.God
	case "/time":
		g.clock = (num(1) - 0.3) * dayLength
		for g.clock < 0 {
			g.clock += dayLength
		}
	case "/spawn":
		if len(f) > 1 {
			if fam := g.pack.Family(f[1]); fam != nil {
				rank := 1
				if len(f) > 2 {
					rank = g.rankIndex(f[2])
				}
				el := g.pack.Element(fam.Elements[len(fam.Elements)-1])
				e := g.makeEnemy(fam, el, rank, c.Level, g.rollTraits(rank), c.X+3, c.Y)
				g.enemies[g.newID()] = e
			}
		}
	default:
		return false
	}
	p.system("dev: " + text)
	return true
}
