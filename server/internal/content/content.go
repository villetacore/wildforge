// Package content loads the data-driven game definitions (items, enemies,
// resources, recipes, world settings) from the content directory.
package content

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

type Item struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	Type    string  `json:"type"` // material, potion, weapon, armor, tool
	Desc    string  `json:"desc"`
	Color   string  `json:"color"`
	Stack   int     `json:"stack"`
	Heal    float64 `json:"heal,omitempty"`
	Attack  int     `json:"attack,omitempty"`
	Defense int     `json:"defense,omitempty"`
	Tool    string  `json:"tool,omitempty"`
	Tier    int     `json:"tier,omitempty"`
}

type Drop struct {
	Item   string  `json:"item"`
	Chance float64 `json:"chance"`
	Min    int     `json:"min"`
	Max    int     `json:"max"`
}

type Enemy struct {
	ID             string  `json:"id"`
	Name           string  `json:"name"`
	Model          string  `json:"model"`
	Texture        string  `json:"texture"`
	Color          string  `json:"color"`
	Scale          float64 `json:"scale"`
	HP             float64 `json:"hp"`
	HPPerLevel     float64 `json:"hp_per_level"`
	Damage         float64 `json:"damage"`
	DamagePerLevel float64 `json:"damage_per_level"`
	Speed          float64 `json:"speed"`
	XP             int     `json:"xp"`
	MinLevel       int     `json:"min_level"`
	Drops          []Drop  `json:"drops"`
}

type SpawnRule struct {
	Terrain string  `json:"terrain"`
	Chance  float64 `json:"chance"`
}

type Resource struct {
	ID          string      `json:"id"`
	Name        string      `json:"name"`
	Item        string      `json:"item"`
	Tool        string      `json:"tool"`
	MinTier     int         `json:"min_tier"`
	Charges     int         `json:"charges"`
	Respawn     float64     `json:"respawn"`
	Model       string      `json:"model"`
	Texture     string      `json:"texture"`
	Color       string      `json:"color"`
	MinDistance float64     `json:"min_distance"`
	Spawn       []SpawnRule `json:"spawn"`
}

type Recipe struct {
	ID          string         `json:"id"`
	Result      string         `json:"result"`
	Qty         int            `json:"qty"`
	Ingredients map[string]int `json:"ingredients"`
}

type Stack struct {
	Item string `json:"item"`
	Qty  int    `json:"qty"`
}

type World struct {
	Size            int     `json:"size"`
	TownRadius      float64 `json:"town_radius"`
	SpawnSafeRadius float64 `json:"spawn_safe_radius"`
	EnemyCount      int     `json:"enemy_count"`
	EnemyRespawn    float64 `json:"enemy_respawn"`
	MaxInventory    int     `json:"max_inventory"`
	GoldPerLevel    [2]int  `json:"gold_per_level"`
	StartInventory  []Stack `json:"start_inventory"`
}

// Pack is everything loaded from the content directory. The exported part
// (items, resources, recipes, enemies) is also sent to clients on login.
type Pack struct {
	Items     []Item     `json:"items"`
	Resources []Resource `json:"resources"`
	Recipes   []Recipe   `json:"recipes"`
	Enemies   []Enemy    `json:"enemies"`
	World     World      `json:"-"`

	items   map[string]*Item
	recipes map[string]*Recipe
}

func readJSON(dir, name string, v any) error {
	data, err := os.ReadFile(filepath.Join(dir, name))
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, v); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	return nil
}

// Load reads and validates the content directory.
func Load(dir string) (*Pack, error) {
	p := &Pack{}
	for name, v := range map[string]any{
		"items.json": &p.Items, "resources.json": &p.Resources, "recipes.json": &p.Recipes,
		"enemies.json": &p.Enemies, "world.json": &p.World,
	} {
		if err := readJSON(dir, name, v); err != nil {
			return nil, err
		}
	}
	p.items = map[string]*Item{}
	for i := range p.Items {
		it := &p.Items[i]
		if it.Stack <= 0 {
			it.Stack = 1
		}
		p.items[it.ID] = it
	}
	p.recipes = map[string]*Recipe{}
	for i := range p.Recipes {
		p.recipes[p.Recipes[i].ID] = &p.Recipes[i]
	}
	if len(p.Resources) > 250 {
		return nil, fmt.Errorf("too many resource kinds (max 250)")
	}
	return p, p.validate()
}

func (p *Pack) validate() error {
	need := func(where, id string) error {
		if p.items[id] == nil {
			return fmt.Errorf("%s: unknown item %q", where, id)
		}
		return nil
	}
	for _, r := range p.Recipes {
		if err := need("recipe "+r.ID, r.Result); err != nil {
			return err
		}
		for id := range r.Ingredients {
			if err := need("recipe "+r.ID, id); err != nil {
				return err
			}
		}
	}
	for _, r := range p.Resources {
		if err := need("resource "+r.ID, r.Item); err != nil {
			return err
		}
	}
	for _, e := range p.Enemies {
		for _, d := range e.Drops {
			if err := need("enemy "+e.ID, d.Item); err != nil {
				return err
			}
		}
	}
	for _, s := range p.World.StartInventory {
		if err := need("world start_inventory", s.Item); err != nil {
			return err
		}
	}
	if p.World.Size < 64 || p.World.MaxInventory <= 0 || len(p.Enemies) == 0 {
		return fmt.Errorf("world.json: size >= 64, max_inventory > 0 and at least one enemy are required")
	}
	return nil
}

func (p *Pack) Item(id string) *Item     { return p.items[id] }
func (p *Pack) Recipe(id string) *Recipe { return p.recipes[id] }

// Resource returns the resource for a node kind (1-based, 0 means none).
func (p *Pack) Resource(kind uint8) *Resource {
	if kind == 0 || int(kind) > len(p.Resources) {
		return nil
	}
	return &p.Resources[kind-1]
}
