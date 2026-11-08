package main

import "fmt"

// Static item and enemy definitions.

type ItemSlot string

const (
	SlotPotion ItemSlot = "potion"
	SlotWeapon ItemSlot = "weapon"
	SlotArmor  ItemSlot = "armor"
	SlotGold   ItemSlot = "gold"
)

type ItemDef struct {
	ID    string
	Name  string
	Slot  ItemSlot
	Value float64 // heal fraction for potions, bonus for weapons and armor
}

var Items = []ItemDef{
	{"potion", "Health Potion", SlotPotion, 0.35},
	{"big_potion", "Great Potion", SlotPotion, 0.8},
	{"wood_sword", "Wooden Sword", SlotWeapon, 3},
	{"iron_sword", "Iron Sword", SlotWeapon, 8},
	{"steel_sword", "Steel Sword", SlotWeapon, 15},
	{"fire_blade", "Fire Blade", SlotWeapon, 26},
	{"leather", "Leather Armor", SlotArmor, 2},
	{"chainmail", "Chainmail", SlotArmor, 5},
	{"plate", "Plate Armor", SlotArmor, 9},
	{"dragon_mail", "Dragon Mail", SlotArmor, 15},
}

var (
	WeaponTiers = []string{"wood_sword", "iron_sword", "steel_sword", "fire_blade"}
	ArmorTiers  = []string{"leather", "chainmail", "plate", "dragon_mail"}
)

var itemByID = func() map[string]*ItemDef {
	m := map[string]*ItemDef{}
	for i := range Items {
		m[Items[i].ID] = &Items[i]
	}
	return m
}()

func Item(id string) *ItemDef { return itemByID[id] }

type CatalogEntry struct {
	Name string   `json:"name"`
	Desc string   `json:"desc"`
	Slot ItemSlot `json:"slot"`
}

// Catalog is sent to clients so they can show item names and descriptions.
func Catalog() map[string]CatalogEntry {
	m := map[string]CatalogEntry{"gold": {"Gold", "Shiny coins", SlotGold}}
	for _, d := range Items {
		var desc string
		switch d.Slot {
		case SlotPotion:
			desc = fmt.Sprintf("Restores %d%% HP", int(d.Value*100))
		case SlotWeapon:
			desc = fmt.Sprintf("+%d attack", int(d.Value))
		case SlotArmor:
			desc = fmt.Sprintf("+%d defense", int(d.Value))
		}
		m[d.ID] = CatalogEntry{d.Name, desc, d.Slot}
	}
	return m
}

type EnemyDef struct {
	Kind        string
	BaseHP      float64
	HPPerLevel  float64
	BaseDmg     float64
	DmgPerLevel float64
	Speed       float64
	XP          int
	MinLevel    int
}

var Enemies = []EnemyDef{
	{"slime", 20, 8, 3, 1.2, 2.0, 8, 1},
	{"wolf", 30, 10, 5, 1.6, 3.6, 13, 3},
	{"skeleton", 45, 13, 7, 2.0, 2.7, 20, 6},
	{"ogre", 110, 22, 12, 3.0, 2.1, 45, 10},
}
