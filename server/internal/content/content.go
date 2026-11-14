// Package content loads the data-driven game definitions (items, bestiary,
// gear generation tables, magic, perks, factions, pets, zones, NPC roles,
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
	Type    string  `json:"type"` // material, potion, food, weapon, armor, tool, tome, pet
	Desc    string  `json:"desc"`
	Color   string  `json:"color"`
	Stack   int     `json:"stack"`
	Price   int     `json:"price"`
	Heal    float64 `json:"heal,omitempty"`
	Mana    float64 `json:"mana,omitempty"`
	Mood    float64 `json:"mood,omitempty"`
	Status  string  `json:"status,omitempty"`
	Cure    bool    `json:"cure,omitempty"`
	Cleanse bool    `json:"cleanse,omitempty"`
	PetHeal float64 `json:"pet_heal,omitempty"`
	Spell   string  `json:"spell,omitempty"`
	Pet     string  `json:"pet,omitempty"`
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
	Zones       []string    `json:"zones,omitempty"`
	Spawn       []SpawnRule `json:"spawn"`
}

type Recipe struct {
	ID          string         `json:"id"`
	Result      string         `json:"result"`
	Qty         int            `json:"qty"`
	Ingredients map[string]int `json:"ingredients"`
}

// Stack is one inventory slot. Template items stack by id; generated gear
// (G != nil) is always a single unique item.
type Stack struct {
	Item string `json:"item"`
	Qty  int    `json:"qty"`
	G    *Gear  `json:"g,omitempty"`
}

type World struct {
	Size            int      `json:"size"`
	TownRadius      float64  `json:"town_radius"`
	SpawnSafeRadius float64  `json:"spawn_safe_radius"`
	EnemyCount      int      `json:"enemy_count"`
	EnemyRespawn    float64  `json:"enemy_respawn"`
	MaxInventory    int      `json:"max_inventory"`
	GoldPerLevel    [2]int   `json:"gold_per_level"`
	StartGold       int      `json:"start_gold"`
	StartSpells     []string `json:"start_spells"`
	StartInventory  []Stack  `json:"start_inventory"`
}

// Stats is an open set of numeric bonuses keyed by stat name (atk, def, hp,
// mana, str, dex, int, vit, crit, sp, speed, fire, ... see game/stats.go).
type Stats map[string]float64

func (s Stats) Add(o Stats, k float64) {
	for key, v := range o {
		s[key] += v * k
	}
}

// ---- bestiary: enemies are assembled from family x element x rank x traits

type Family struct {
	ID       string   `json:"id"`
	Name     string   `json:"name"`
	Model    string   `json:"model"`
	Texture  string   `json:"texture"`
	Color    string   `json:"color"`
	Scale    float64  `json:"scale"`
	HP       float64  `json:"hp"`
	HPLvl    float64  `json:"hp_lvl"`
	Dmg      float64  `json:"dmg"`
	DmgLvl   float64  `json:"dmg_lvl"`
	Speed    float64  `json:"speed"`
	XP       int      `json:"xp"`
	Armor    float64  `json:"armor"`
	Range    float64  `json:"range"`
	Bolt     string   `json:"bolt"`
	Tameable bool     `json:"tameable"`
	Faction  string   `json:"faction"`
	Elements []string `json:"elements"`
	Drops    []Drop   `json:"drops"`
}

type Element struct {
	ID        string  `json:"id"`
	Name      string  `json:"name"`
	Color     string  `json:"color"`
	Status    string  `json:"status"`
	Chance    float64 `json:"chance"`
	Dmg       float64 `json:"dmg"`
	HP        float64 `json:"hp"`
	Armor     float64 `json:"armor"`
	Lifesteal float64 `json:"lifesteal"`
	Drop      string  `json:"drop"`
	Weight    float64 `json:"weight"`
}

type Rank struct {
	ID     string  `json:"id"`
	Name   string  `json:"name"`
	HP     float64 `json:"hp"`
	Dmg    float64 `json:"dmg"`
	Scale  float64 `json:"scale"`
	XP     float64 `json:"xp"`
	Traits [2]int  `json:"traits"`
	Loot   float64 `json:"loot"`
	Weight float64 `json:"weight"`
}

type Trait struct {
	ID          string  `json:"id"`
	Name        string  `json:"name"`
	HP          float64 `json:"hp"`
	Dmg         float64 `json:"dmg"`
	Speed       float64 `json:"speed"`
	Scale       float64 `json:"scale"`
	XP          float64 `json:"xp"`
	Armor       float64 `json:"armor"`
	Regen       float64 `json:"regen"`
	Lifesteal   float64 `json:"lifesteal"`
	Thorns      float64 `json:"thorns"`
	Dodge       float64 `json:"dodge"`
	AttackSpeed float64 `json:"attack_speed"`
	Status      string  `json:"status"`
	Chance      float64 `json:"chance"`
	Explode     float64 `json:"explode"`
	Gold        float64 `json:"gold"`
	Loot        float64 `json:"loot"`
}

type Bestiary struct {
	Families  []Family  `json:"families"`
	Elements  []Element `json:"elements"`
	Ranks     []Rank    `json:"ranks"`
	Traits    []Trait   `json:"traits"`
	BossNames []string  `json:"boss_names"`
}

// ---- gear generation tables

type GearBase struct {
	ID    string `json:"id"`
	Name  string `json:"name"`
	Slot  string `json:"slot"`
	Model string `json:"model"`
	Mats  string `json:"mats"`
	Level int    `json:"level"`
	Stats Stats  `json:"stats"`
	Qty   int    `json:"qty"` // material units needed to forge
}

type Material struct {
	ID    string  `json:"id"`
	Name  string  `json:"name"`
	Kind  string  `json:"kind"`
	Tier  float64 `json:"tier"`
	Level int     `json:"level"`
	Color string  `json:"color"`
	Item  string  `json:"item"`
	Price int     `json:"price"`
}

type Quality struct {
	ID     string  `json:"id"`
	Name   string  `json:"name"`
	Mult   float64 `json:"mult"`
	Weight float64 `json:"weight"`
}

type Rarity struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	Color   string  `json:"color"`
	Affixes int     `json:"affixes"`
	Mult    float64 `json:"mult"`
	Weight  float64 `json:"weight"`
}

type Affix struct {
	ID    string   `json:"id"`
	Name  string   `json:"name"`
	Pos   string   `json:"pos"`
	Stats Stats    `json:"stats"`
	Slots []string `json:"slots"`
	Level int      `json:"level"`
}

type Enchant struct {
	ID    string         `json:"id"`
	Name  string         `json:"name"`
	Stats Stats          `json:"stats"`
	Slots []string       `json:"slots"`
	Cost  map[string]int `json:"cost"`
	Gold  int            `json:"gold"`
	Level int            `json:"level"`
}

type GearTables struct {
	Bases       []GearBase `json:"bases"`
	Materials   []Material `json:"materials"`
	Qualities   []Quality  `json:"qualities"`
	Rarities    []Rarity   `json:"rarities"`
	Affixes     []Affix    `json:"affixes"`
	Enchants    []Enchant  `json:"enchants"`
	LegendNames []string   `json:"legend_names"`
}

// Gear is one generated (or forged) equipment piece. Stats and Lines are
// derived from the other fields by game.refreshGear.
type Gear struct {
	Base    string   `json:"base"`
	Mat     string   `json:"mat"`
	Quality int      `json:"quality"`
	Rarity  int      `json:"rarity"`
	Level   int      `json:"level"`
	Affixes []string `json:"affixes,omitempty"`
	Enchant string   `json:"enchant,omitempty"`
	Custom  string   `json:"custom,omitempty"`
	Legend  string   `json:"legend,omitempty"`
	Crafter string   `json:"crafter,omitempty"`

	Name  string   `json:"name"`
	Slot  string   `json:"slot"`
	Model string   `json:"model"`
	Color string   `json:"color"`
	Tint  string   `json:"tint"`
	Value int      `json:"value"`
	Stats Stats    `json:"stats"`
	Lines []string `json:"lines"`
}

// ---- magic

type Spell struct {
	ID     string  `json:"id"`
	Name   string  `json:"name"`
	School string  `json:"school"`
	Kind   string  `json:"kind"` // bolt, dot, aoe_target, aoe_self, chain, heal, buff, drain
	Mana   float64 `json:"mana"`
	CD     float64 `json:"cd"`
	Range  float64 `json:"range"`
	Power  float64 `json:"power"`
	Scale  float64 `json:"scale"`
	Radius float64 `json:"radius,omitempty"`
	Chain  int     `json:"chain,omitempty"`
	Status string  `json:"status,omitempty"`
	Chance float64 `json:"chance,omitempty"`
	Color  string  `json:"color"`
	Level  int     `json:"level"`
	Price  int     `json:"price"`
	Desc   string  `json:"desc"`
}

type StatusDef struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	Color   string  `json:"color"`
	Dur     float64 `json:"dur"`
	Dot     float64 `json:"dot,omitempty"`
	Hot     float64 `json:"hot,omitempty"`
	Slow    float64 `json:"slow,omitempty"`
	Stun    bool    `json:"stun,omitempty"`
	Taken   float64 `json:"taken,omitempty"`
	Dealt   float64 `json:"dealt,omitempty"`
	Absorb  float64 `json:"absorb,omitempty"`
	XP      float64 `json:"xp,omitempty"`
	Bad     bool    `json:"bad,omitempty"`
	Disease bool    `json:"disease,omitempty"`
	Stats   Stats   `json:"stats,omitempty"`
	Desc    string  `json:"desc,omitempty"`
}

type Magic struct {
	Spells   []Spell     `json:"spells"`
	Statuses []StatusDef `json:"statuses"`
}

// ---- perks

type PerkTree struct {
	ID    string `json:"id"`
	Name  string `json:"name"`
	Color string `json:"color"`
}

type Perk struct {
	ID    string `json:"id"`
	Tree  string `json:"tree"`
	Tier  int    `json:"tier"`
	Name  string `json:"name"`
	Desc  string `json:"desc"`
	Stats Stats  `json:"stats"`
}

type Perks struct {
	Trees []PerkTree `json:"trees"`
	Perks []Perk     `json:"perks"`
}

// ---- factions

type Faction struct {
	ID      string   `json:"id"`
	Name    string   `json:"name"`
	Color   string   `json:"color"`
	Desc    string   `json:"desc"`
	Allies  []string `json:"allies"`
	Enemies []string `json:"enemies"`
	Hostile bool     `json:"hostile"`
}

type RepRank struct {
	Name  string  `json:"name"`
	Min   int     `json:"min"`
	Color string  `json:"color"`
	Price float64 `json:"price"`
}

type Factions struct {
	Factions []Faction `json:"factions"`
	Ranks    []RepRank `json:"ranks"`
}

// ---- pets

type PetDef struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	Model   string  `json:"model"`
	Texture string  `json:"texture"`
	Color   string  `json:"color"`
	Scale   float64 `json:"scale"`
	HP      float64 `json:"hp"`
	Dmg     float64 `json:"dmg"`
	Speed   float64 `json:"speed"`
	Fly     bool    `json:"fly"`
	Ability string  `json:"ability"`
	Desc    string  `json:"desc"`
}

type PetAbility struct {
	ID     string  `json:"id"`
	Name   string  `json:"name"`
	CD     float64 `json:"cd"`
	Status string  `json:"status"`
	Power  float64 `json:"power"`
	Radius float64 `json:"radius"`
	Taunt  bool    `json:"taunt"`
	MF     float64 `json:"mf"`
}

type Pets struct {
	Pets      []PetDef     `json:"pets"`
	Abilities []PetAbility `json:"abilities"`
}

// ---- zones and NPC roles

type Town struct {
	Name    string  `json:"name"`
	Style   string  `json:"style"`
	Faction string  `json:"faction"`
	Radius  float64 `json:"radius"`
}

type DungeonDef struct {
	Name     string   `json:"name"`
	Theme    string   `json:"theme"`
	Families []string `json:"families"`
	Level    int      `json:"level"`
}

type Zone struct {
	ID       string             `json:"id"`
	Name     string             `json:"name"`
	Biome    string             `json:"biome"`
	CX       float64            `json:"cx"`
	CY       float64            `json:"cy"`
	MinLevel int                `json:"min_level"`
	MaxLevel int                `json:"max_level"`
	Fog      string             `json:"fog"`
	Ambient  string             `json:"ambient"`
	Color    string             `json:"color"`
	Town     Town               `json:"town"`
	Families []string           `json:"families"`
	Elements map[string]float64 `json:"elements"`
	Dungeon  DungeonDef         `json:"dungeon"`
}

type Role struct {
	ID       string   `json:"id"`
	Title    string   `json:"title"`
	Outfit   string   `json:"outfit"`
	Color    string   `json:"color"`
	Marker   string   `json:"marker"`
	Faction  string   `json:"faction"`
	Names    []string `json:"names"`
	Greeting string   `json:"greeting"`
	Shop     []string `json:"shop"`
}

type NPCs struct {
	Roles  []Role   `json:"roles"`
	Rumors []string `json:"rumors"`
}

// Pack is everything loaded from the content directory. The exported part
// (items, resources, recipes, spells, zones, factions) is also sent to
// clients on login.
type Pack struct {
	Items     []Item     `json:"items"`
	Resources []Resource `json:"resources"`
	Recipes   []Recipe   `json:"recipes"`
	Spells    []Spell    `json:"spells"`
	Zones     []Zone     `json:"zones"`
	Factions  []Faction  `json:"factions"`
	World     World      `json:"-"`
	Bestiary  Bestiary   `json:"-"`
	Gear      GearTables `json:"-"`
	Magic     Magic      `json:"-"`
	Perks     Perks      `json:"-"`
	FactionsT Factions   `json:"-"`
	Pets      Pets       `json:"-"`
	NPCs      NPCs       `json:"-"`

	items     map[string]*Item
	recipes   map[string]*Recipe
	families  map[string]*Family
	elements  map[string]*Element
	traits    map[string]*Trait
	bases     map[string]*GearBase
	materials map[string]*Material
	affixes   map[string]*Affix
	enchants  map[string]*Enchant
	spells    map[string]*Spell
	statuses  map[string]*StatusDef
	perks     map[string]*Perk
	factions  map[string]*Faction
	pets      map[string]*PetDef
	abilities map[string]*PetAbility
	roles     map[string]*Role
	zones     map[string]int
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

func index[T any](list []T, id func(*T) string) map[string]*T {
	m := make(map[string]*T, len(list))
	for i := range list {
		m[id(&list[i])] = &list[i]
	}
	return m
}

// Load reads and validates the content directory.
func Load(dir string) (*Pack, error) {
	p := &Pack{}
	var zones struct {
		Zones []Zone `json:"zones"`
	}
	for name, v := range map[string]any{
		"items.json": &p.Items, "resources.json": &p.Resources, "recipes.json": &p.Recipes,
		"world.json": &p.World, "bestiary.json": &p.Bestiary, "gear.json": &p.Gear,
		"magic.json": &p.Magic, "perks.json": &p.Perks, "factions.json": &p.FactionsT,
		"pets.json": &p.Pets, "zones.json": &zones, "npcs.json": &p.NPCs,
	} {
		if err := readJSON(dir, name, v); err != nil {
			return nil, err
		}
	}
	p.Zones = zones.Zones
	p.Spells = p.Magic.Spells
	p.Factions = p.FactionsT.Factions
	for i := range p.Items {
		if p.Items[i].Stack <= 0 {
			p.Items[i].Stack = 1
		}
	}
	p.items = index(p.Items, func(v *Item) string { return v.ID })
	p.recipes = index(p.Recipes, func(v *Recipe) string { return v.ID })
	p.families = index(p.Bestiary.Families, func(v *Family) string { return v.ID })
	p.elements = index(p.Bestiary.Elements, func(v *Element) string { return v.ID })
	p.traits = index(p.Bestiary.Traits, func(v *Trait) string { return v.ID })
	p.bases = index(p.Gear.Bases, func(v *GearBase) string { return v.ID })
	p.materials = index(p.Gear.Materials, func(v *Material) string { return v.ID })
	p.affixes = index(p.Gear.Affixes, func(v *Affix) string { return v.ID })
	p.enchants = index(p.Gear.Enchants, func(v *Enchant) string { return v.ID })
	p.spells = index(p.Magic.Spells, func(v *Spell) string { return v.ID })
	p.statuses = index(p.Magic.Statuses, func(v *StatusDef) string { return v.ID })
	p.perks = index(p.Perks.Perks, func(v *Perk) string { return v.ID })
	p.factions = index(p.FactionsT.Factions, func(v *Faction) string { return v.ID })
	p.pets = index(p.Pets.Pets, func(v *PetDef) string { return v.ID })
	p.abilities = index(p.Pets.Abilities, func(v *PetAbility) string { return v.ID })
	p.roles = index(p.NPCs.Roles, func(v *Role) string { return v.ID })
	p.zones = map[string]int{}
	for i, z := range p.Zones {
		p.zones[z.ID] = i
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
	for _, f := range p.Bestiary.Families {
		for _, d := range f.Drops {
			if err := need("family "+f.ID, d.Item); err != nil {
				return err
			}
		}
		for _, e := range f.Elements {
			if p.elements[e] == nil {
				return fmt.Errorf("family %s: unknown element %q", f.ID, e)
			}
		}
	}
	for _, e := range p.Bestiary.Elements {
		if e.Drop != "" {
			if err := need("element "+e.ID, e.Drop); err != nil {
				return err
			}
		}
		if e.Status != "" && p.statuses[e.Status] == nil {
			return fmt.Errorf("element %s: unknown status %q", e.ID, e.Status)
		}
	}
	for _, t := range p.Bestiary.Traits {
		if t.Status != "" && p.statuses[t.Status] == nil {
			return fmt.Errorf("trait %s: unknown status %q", t.ID, t.Status)
		}
	}
	for _, m := range p.Gear.Materials {
		if err := need("material "+m.ID, m.Item); err != nil {
			return err
		}
	}
	for _, e := range p.Gear.Enchants {
		for id := range e.Cost {
			if err := need("enchant "+e.ID, id); err != nil {
				return err
			}
		}
	}
	for _, s := range p.Magic.Spells {
		if s.Status != "" && p.statuses[s.Status] == nil {
			return fmt.Errorf("spell %s: unknown status %q", s.ID, s.Status)
		}
	}
	for _, it := range p.Items {
		if it.Spell != "" && p.spells[it.Spell] == nil {
			return fmt.Errorf("item %s: unknown spell %q", it.ID, it.Spell)
		}
		if it.Pet != "" && p.pets[it.Pet] == nil {
			return fmt.Errorf("item %s: unknown pet %q", it.ID, it.Pet)
		}
		if it.Status != "" && p.statuses[it.Status] == nil {
			return fmt.Errorf("item %s: unknown status %q", it.ID, it.Status)
		}
	}
	for _, pt := range p.Pets.Pets {
		if p.abilities[pt.Ability] == nil {
			return fmt.Errorf("pet %s: unknown ability %q", pt.ID, pt.Ability)
		}
	}
	for _, z := range p.Zones {
		for _, f := range append(append([]string{}, z.Families...), z.Dungeon.Families...) {
			if p.families[f] == nil {
				return fmt.Errorf("zone %s: unknown family %q", z.ID, f)
			}
		}
		if p.factions[z.Town.Faction] == nil {
			return fmt.Errorf("zone %s: unknown faction %q", z.ID, z.Town.Faction)
		}
	}
	for _, r := range p.NPCs.Roles {
		for _, id := range r.Shop {
			if err := need("npc role "+r.ID, id); err != nil {
				return err
			}
		}
	}
	for _, s := range p.World.StartInventory {
		if err := need("world start_inventory", s.Item); err != nil {
			return err
		}
	}
	if p.World.Size < 64 || p.World.MaxInventory <= 0 || len(p.Bestiary.Families) == 0 || len(p.Zones) == 0 {
		return fmt.Errorf("world.json: size >= 64, max_inventory > 0, at least one family and one zone are required")
	}
	return nil
}

func (p *Pack) Item(id string) *Item            { return p.items[id] }
func (p *Pack) Recipe(id string) *Recipe        { return p.recipes[id] }
func (p *Pack) Family(id string) *Family        { return p.families[id] }
func (p *Pack) Element(id string) *Element      { return p.elements[id] }
func (p *Pack) Trait(id string) *Trait          { return p.traits[id] }
func (p *Pack) Base(id string) *GearBase        { return p.bases[id] }
func (p *Pack) Material(id string) *Material    { return p.materials[id] }
func (p *Pack) Affix(id string) *Affix          { return p.affixes[id] }
func (p *Pack) EnchantDef(id string) *Enchant   { return p.enchants[id] }
func (p *Pack) Spell(id string) *Spell          { return p.spells[id] }
func (p *Pack) Status(id string) *StatusDef     { return p.statuses[id] }
func (p *Pack) Perk(id string) *Perk            { return p.perks[id] }
func (p *Pack) Faction(id string) *Faction      { return p.factions[id] }
func (p *Pack) PetDef(id string) *PetDef        { return p.pets[id] }
func (p *Pack) Ability(id string) *PetAbility   { return p.abilities[id] }
func (p *Pack) Role(id string) *Role            { return p.roles[id] }
func (p *Pack) ZoneIndex(id string) (int, bool) { i, ok := p.zones[id]; return i, ok }

// Resource returns the resource for a node kind (1-based, 0 means none).
func (p *Pack) Resource(kind uint8) *Resource {
	if kind == 0 || int(kind) > len(p.Resources) {
		return nil
	}
	return &p.Resources[kind-1]
}
