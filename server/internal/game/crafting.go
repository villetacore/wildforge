package game

import (
	"fmt"
	"slices"
	"sort"
	"strings"
)

// craft consumes a recipe's ingredients and adds the result. The whole
// operation is rolled back if the result does not fit.
func (g *Game) craft(p *Player, recipeID string) {
	r := g.pack.Recipe(recipeID)
	if r == nil {
		return
	}
	inv := p.Ch.inv(g.pack)
	var missing []string
	ids := make([]string, 0, len(r.Ingredients))
	for id := range r.Ingredients {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		if have, need := inv.Count(id), r.Ingredients[id]; have < need {
			missing = append(missing, fmt.Sprintf("%d %s", need-have, g.pack.Item(id).Name))
		}
	}
	if len(missing) > 0 {
		p.system("Missing: " + strings.Join(missing, ", "))
		return
	}
	backup := slices.Clone(p.Ch.Inventory)
	for _, id := range ids {
		inv.Remove(id, r.Ingredients[id])
	}
	if inv.Add(r.Result, r.Qty) > 0 {
		p.Ch.Inventory = backup
		p.system("Inventory full.")
		return
	}
	name := g.pack.Item(r.Result).Name
	g.addFx("loot", p.Ch.X, p.Ch.Y, "Crafted "+name)
	p.system("Crafted " + name + ".")
}
