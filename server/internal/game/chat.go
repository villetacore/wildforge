package game

import (
	"fmt"
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
		p.system(fmt.Sprintf("Level %d | XP %d/%d | ATK %d | DEF %d | Gold %d | Kills %d",
			c.Level, c.XP, XPForNext(c.Level), c.Attack(g.pack), c.Defense(g.pack), c.Gold, c.Kills))
		return
	case "/help":
		p.system("WASD move, Space attack, E gather, C craft, Q potion, 1-9 use item, Enter chat. Commands: /who /stats /help")
		return
	}
	g.broadcast(protocol.Chat{T: "chat", From: p.Name, Text: text})
}
