// Package world generates and queries the tile map: zones and biomes, a river,
// towns with buildings, roads and bridges, procedural dungeons, and gatherable
// resource nodes (trees, rocks, ores), which also block movement while alive.
package world

import (
	"math"
	"strings"

	"github.com/villetacore/wildforge/server/internal/content"
)

type Terrain uint8

const (
	Grass Terrain = iota
	Water
	Sand
	Forest
	Road
	Rocky
	Wall
	DungeonFloor
	Portal
	Chest
	Snow
	Swamp
	Ash
	Cobble
	Building
	Lava
	Bridge
	CityWall
)

var terrainNames = map[string]Terrain{
	"grass": Grass, "water": Water, "sand": Sand, "forest": Forest, "road": Road, "rocky": Rocky,
	"snow": Snow, "swamp": Swamp, "ash": Ash, "cobble": Cobble,
}

// TownInfo describes a settlement: a safe zone without monsters or resources.
type TownInfo struct {
	Name    string  `json:"name"`
	Zone    int     `json:"zone"`
	X       float64 `json:"x"`
	Y       float64 `json:"y"`
	R       float64 `json:"r"`
	Style   string  `json:"style"`
	Faction string  `json:"faction"`
}

type World struct {
	Heights    []float64
	Portals    []PortalLink
	Size       int
	Terrain    []Terrain
	Nodes      []uint8 // resource kind per tile, 1-based index into content resources, 0 = none
	Charges    []int   // remaining gathers per node; 0 = depleted
	Zone       []uint8 // zone index per tile
	SpawnX     float64
	SpawnY     float64
	Structures []Structure
	NPCSpots   []NPCSpot
	Towns      []TownInfo
	Dungeons   []*Dungeon

	pack *content.Pack
	seed uint32
}

func smoothstep(t float64) float64 {
	t = math.Max(0, math.Min(1, t))
	return t * t * (3 - 2*t)
}

// RiverX is the river's centre column for a row. It meanders across the
// map and runs calmly through the capital.
func (w *World) RiverX(y int) float64 {
	n := float64(w.Size)
	c := n / 2
	dy := float64(y) - c
	calm := smoothstep((math.Abs(dy) - 30) / 40)
	return c + 7 + 2*math.Sin(dy/14) + calm*(22*math.Sin(dy/57)+9*math.Sin(dy/23+1))
}

// Generate builds the continent from noise, carves the river, lays out towns
// and roads, digs the dungeons and scatters resource nodes according to the
// spawn rules in the content pack.
func Generate(seed uint32, pack *content.Pack) *World {
	n := pack.World.Size
	w := &World{
		Size: n, Terrain: make([]Terrain, n*n), Nodes: make([]uint8, n*n), Charges: make([]int, n*n),
		Zone: make([]uint8, n*n), pack: pack, seed: seed,
	}
	c := float64(n) / 2
	scale := float64(n) / 256 // keep feature sizes similar for other map sizes
	for y := 0; y < n; y++ {
		for x := 0; x < n; x++ {
			fx, fy := float64(x)/scale, float64(y)/scale
			e := FBM(fx/48, fy/48, seed)
			dx, dy := (float64(x)-c)/c, (float64(y)-c)/c
			d := math.Sqrt(dx*dx + dy*dy)
			e = e + 0.24 - math.Pow(d, 2.6)*0.62
			z := w.zoneAt(x, y)
			w.Zone[y*n+x] = uint8(z)
			moist := FBM(fx/32+500, fy/32+500, seed^0xBEEF)
			rock := FBM(fx/16+900, fy/16+900, seed^0xC0DE)
			special := FBM(float64(x)/9+300, float64(y)/9+300, seed^0xFACE)
			t := Grass
			switch {
			case e < 0.32:
				t = Water
			case e < 0.355:
				t = Sand
				if pack.Zones[z].Biome == "snow" {
					t = Rocky
				}
			default:
				t = biomeTerrain(pack.Zones[z].Biome, e, moist, rock, special)
			}
			w.Terrain[y*n+x] = t
		}
	}
	w.carveRiver()
	w.placeTowns()
	w.layRoads()
	w.buildTowns()
	w.generateDungeons(seed)
	w.scatterNodes()
	w.settleSpots()
	w.generateHeights(seed)
	return w
}

func biomeTerrain(biome string, e, moist, rock, special float64) Terrain {
	switch biome {
	case "forest":
		if rock > 0.66 && e > 0.5 {
			return Rocky
		}
		if moist > 0.42 {
			return Forest
		}
	case "snow":
		if rock > 0.56 {
			return Rocky
		}
		return Snow
	case "ash":
		if special > 0.72 {
			return Lava
		}
		if rock > 0.52 {
			return Rocky
		}
		return Ash
	case "swamp":
		if special > 0.66 {
			return Water
		}
		if moist > 0.6 {
			return Forest
		}
		return Swamp
	case "desert":
		if rock > 0.64 {
			return Rocky
		}
		if special > 0.8 {
			return Grass // oasis
		}
		return Sand
	default: // meadow
		if rock > 0.62 && e > 0.5 {
			return Rocky
		}
		if moist > 0.6 {
			return Forest
		}
	}
	return Grass
}

// zoneAt picks the nearest zone centre with a noisy distance so borders wiggle.
func (w *World) zoneAt(x, y int) int {
	n := float64(w.Size)
	best, bestD := 0, math.MaxFloat64
	for i, z := range w.pack.Zones {
		d := math.Hypot(float64(x)-z.CX*n, float64(y)-z.CY*n)
		d *= 0.8 + 0.4*FBM(float64(x)/70+float64(i)*13, float64(y)/70, w.seed^uint32(0x2077+i*31))
		if z.Town.Style == "capital" {
			d *= 0.85
		}
		if d < bestD {
			best, bestD = i, d
		}
	}
	return best
}

func (w *World) carveRiver() {
	for y := 0; y < w.Size; y++ {
		cx := w.RiverX(y)
		half := 2.5 + 0.8*math.Sin(float64(y)/31)
		for x := int(cx - half - 1); x <= int(cx+half+1); x++ {
			if !w.InBounds(x, y) {
				continue
			}
			d := math.Abs(float64(x) + 0.5 - cx)
			i := w.Index(x, y)
			if d < half {
				w.Terrain[i] = Water
			} else if d < half+1.2 && w.Terrain[i] != Water && w.pack.Zones[w.Zone[i]].Biome != "snow" {
				w.Terrain[i] = Sand
			}
		}
	}
}

// placeTowns finds a dry spot near each zone's centre. The capital sits at the
// map centre and is the spawn point.
func (w *World) placeTowns() {
	n := float64(w.Size)
	for zi, z := range w.pack.Zones {
		tx, ty := z.CX*n, z.CY*n
		if z.Town.Style == "capital" {
			tx, ty = n/2, n/2
		} else {
			bestScore := -1.0
			for r := 0.0; r < 40 && bestScore < 0.97; r += 2 {
				for a := 0.0; a < 2*math.Pi; a += 0.35 {
					x, y := z.CX*n+math.Cos(a)*r, z.CY*n+math.Sin(a)*r
					score := w.landScore(int(x), int(y), int(z.Town.Radius)+3)
					if score > bestScore {
						bestScore, tx, ty = score, x, y
					}
				}
			}
		}
		w.Towns = append(w.Towns, TownInfo{
			Name: z.Town.Name, Zone: zi, X: math.Floor(tx) + .5, Y: math.Floor(ty) + .5, R: z.Town.Radius,
			Style: z.Town.Style, Faction: z.Town.Faction,
		})
	}
	w.SpawnX, w.SpawnY = w.Towns[0].X, w.Towns[0].Y
	for _, t := range w.Towns {
		if t.Style == "capital" {
			w.SpawnX, w.SpawnY = t.X+20, t.Y+3 // old town square, south of the statue
		}
	}
}

func (w *World) landScore(cx, cy, r int) float64 {
	land, total := 0, 0
	for y := cy - r; y <= cy+r; y += 2 {
		for x := cx - r; x <= cx+r; x += 2 {
			total++
			if w.InBounds(x, y) && w.Terrain[w.Index(x, y)] != Water && w.Terrain[w.Index(x, y)] != Lava {
				land++
			}
		}
	}
	return float64(land) / float64(max(total, 1))
}

// TownAt returns the town whose safe area contains (x, y).
func (w *World) TownAt(x, y float64) (int, bool) {
	for i, t := range w.Towns {
		if math.Hypot(x-t.X, y-t.Y) < t.R+2 {
			return i, true
		}
	}
	return -1, false
}

// layRoads connects the capital to every other town. Roads cross water on
// bridges.
func (w *World) layRoads() {
	cap := w.Towns[0]
	for _, t := range w.Towns[1:] {
		w.road(cap.X, cap.Y, t.X, t.Y, 2)
	}
	// The capital's main cross streets run to the map edge so the realm feels connected.
	c := w.Size / 2
	for i := 0; i < w.Size; i++ {
		for off := 0; off < 2; off++ {
			for _, p := range [][2]int{{i, c + off}, {c + off, i}} {
				idx := w.Index(p[0], p[1])
				if math.Hypot(float64(p[0]-c), float64(p[1]-c)) > 60 {
					continue
				}
				if w.Terrain[idx] == Water {
					if w.nearLand(p[0], p[1]) {
						w.Terrain[idx] = Bridge
					}
				} else if w.Terrain[idx] != Lava && w.Terrain[idx] != Bridge {
					w.Terrain[idx] = Road
				}
			}
		}
	}
}

func (w *World) nearLand(x, y int) bool {
	for d := 1; d < 9; d++ {
		for _, o := range [][2]int{{d, 0}, {-d, 0}, {0, d}, {0, -d}} {
			if w.InBounds(x+o[0], y+o[1]) && w.Terrain[w.Index(x+o[0], y+o[1])] != Water {
				return true
			}
		}
	}
	return false
}

func (w *World) road(ax, ay, bx, by float64, width int) {
	steps := int(math.Hypot(bx-ax, by-ay) * 2)
	nx, ny := -(by - ay), bx-ax
	l := math.Hypot(nx, ny)
	nx, ny = nx/l, ny/l
	for s := 0; s <= steps; s++ {
		t := float64(s) / float64(steps)
		wob := math.Sin(t*math.Pi) * 10 * math.Sin(t*7+ax*0.1)
		x := ax + (bx-ax)*t + nx*wob
		y := ay + (by-ay)*t + ny*wob
		for oy := 0; oy < width; oy++ {
			for ox := 0; ox < width; ox++ {
				xi, yi := int(x)+ox, int(y)+oy
				if !w.InBounds(xi, yi) {
					continue
				}
				i := w.Index(xi, yi)
				switch w.Terrain[i] {
				case Water:
					w.Terrain[i] = Bridge
				case Lava:
					w.Terrain[i] = Ash
				case Bridge, Cobble, Building, CityWall:
				default:
					w.Terrain[i] = Road
				}
			}
		}
	}
}

func (w *World) scatterNodes() {
	n := w.Size
	for y := 1; y < n-1; y++ {
		for x := 1; x < n-1; x++ {
			idx := y*n + x
			t := w.Terrain[idx]
			if t == Water || t == Road || t >= Wall && t != Snow && t != Swamp && t != Ash {
				continue
			}
			if _, inTown := w.TownAt(float64(x)+.5, float64(y)+.5); inTown || w.Underground(float64(x), float64(y)) || w.nearPortal(x, y) {
				continue
			}
			c := float64(n) / 2
			dist := math.Hypot(float64(x)-c, float64(y)-c) / c
			zone := w.pack.Zones[w.Zone[idx]].ID
		resources:
			for ri, r := range w.pack.Resources {
				if dist < r.MinDistance {
					continue
				}
				if len(r.Zones) > 0 && !contains(r.Zones, zone) {
					continue
				}
				roll := Hash(int32(x), int32(y), w.seed^uint32(0x51ED+ri*7919))
				for _, rule := range r.Spawn {
					if terrainNames[rule.Terrain] == t && roll < rule.Chance {
						w.Nodes[idx] = uint8(ri + 1)
						w.Charges[idx] = r.Charges
						break resources
					}
				}
			}
		}
	}
}

func (w *World) nearPortal(x, y int) bool {
	for _, p := range w.Portals {
		if math.Abs(float64(x)-p.X) < 4 && math.Abs(float64(y)-p.Y) < 4 {
			return true
		}
	}
	return false
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}

func (w *World) Index(x, y int) int { return y*w.Size + x }

func (w *World) InBounds(x, y int) bool { return x >= 0 && y >= 0 && x < w.Size && y < w.Size }

// Solid reports terrain that blocks both movement and line of sight.
func Solid(t Terrain) bool { return t == Wall || t == Building || t == CityWall }

// Blocked reports whether a tile cannot be walked on.
func (w *World) Blocked(x, y int) bool {
	if !w.InBounds(x, y) {
		return true
	}
	i := w.Index(x, y)
	t := w.Terrain[i]
	return t == Water || t == Lava || Solid(t) || (w.Nodes[i] != 0 && w.Charges[i] > 0)
}

// Free reports whether a circle of radius 0.3 at (x, y) fits on walkable tiles.
func (w *World) Free(x, y float64) bool {
	const r = 0.3
	for _, o := range [4][2]float64{{-r, -r}, {r, -r}, {-r, r}, {r, r}} {
		if w.Blocked(int(math.Floor(x+o[0])), int(math.Floor(y+o[1]))) {
			return false
		}
	}
	return true
}

// Step moves with axis-separated collision so entities slide along walls.
func (w *World) Step(x, y, dx, dy float64) (float64, float64) {
	if w.Free(x+dx, y) {
		x += dx
	}
	if w.Free(x, y+dy) {
		y += dy
	}
	return x, y
}

// ZoneOf returns the zone index for a position.
func (w *World) ZoneOf(x, y float64) int {
	xi, yi := int(x), int(y)
	if !w.InBounds(xi, yi) {
		return 0
	}
	return int(w.Zone[w.Index(xi, yi)])
}

// NearestNode finds the closest alive resource node within radius of (x, y).
func (w *World) NearestNode(x, y, radius float64) (int, bool) {
	best, bestD := -1, radius
	r := int(math.Ceil(radius))
	for ty := int(y) - r; ty <= int(y)+r; ty++ {
		for tx := int(x) - r; tx <= int(x)+r; tx++ {
			if !w.InBounds(tx, ty) {
				continue
			}
			i := w.Index(tx, ty)
			if w.Nodes[i] == 0 || w.Charges[i] <= 0 {
				continue
			}
			if d := math.Hypot(float64(tx)+0.5-x, float64(ty)+0.5-y); d <= bestD {
				best, bestD = i, d
			}
		}
	}
	return best, best >= 0
}

func encode[T ~uint8](cells []T) string {
	var b strings.Builder
	b.Grow(len(cells))
	for _, c := range cells {
		b.WriteByte('0' + byte(c))
	}
	return b.String()
}

func (w *World) EncodeTerrain() string { return encode(w.Terrain) }
func (w *World) EncodeNodes() string   { return encode(w.Nodes) }
func (w *World) EncodeZones() string   { return encode(w.Zone) }

// EncodeRect encodes the terrain of a rectangle row by row.
func (w *World) EncodeRect(x0, y0, size int) string {
	cells := make([]Terrain, 0, size*size)
	for y := y0; y < y0+size; y++ {
		cells = append(cells, w.Terrain[w.Index(x0, y):w.Index(x0, y)+size]...)
	}
	return encode(cells)
}

// Depleted lists node tiles that are currently used up.
func (w *World) Depleted() []int {
	out := []int{}
	for i, k := range w.Nodes {
		if k != 0 && w.Charges[i] <= 0 {
			out = append(out, i)
		}
	}
	return out
}

func (w *World) generateHeights(seed uint32) {
	w.Heights = make([]float64, (w.Size+1)*(w.Size+1))
	var castle *Structure
	for i := range w.Structures {
		if w.Structures[i].Kind == "castle_hill" {
			castle = &w.Structures[i]
		}
	}
	for y := 0; y <= w.Size; y++ {
		for x := 0; x <= w.Size; x++ {
			h := 0.0
			wet := false
			for dy := -1; dy <= 0; dy++ {
				for dx := -1; dx <= 0; dx++ {
					if !w.InBounds(x+dx, y+dy) {
						wet = true
						continue
					}
					switch w.Terrain[w.Index(x+dx, y+dy)] {
					case Water, Bridge:
						wet = true
					}
				}
			}
			if !wet {
				biome := w.pack.Zones[w.Zone[w.Index(min(x, w.Size-1), min(y, w.Size-1))]].Biome
				amp := map[string]float64{"snow": 22, "ash": 13, "swamp": 2, "desert": 6, "forest": 12}[biome]
				if amp == 0 {
					amp = 10
				}
				base := math.Max(0, FBM(float64(x)/35, float64(y)/35, seed+812)-.25)
				if biome == "desert" {
					base += 0.12 * (math.Sin(float64(x)/6+FBM(float64(x)/20, float64(y)/20, seed)*6) + 1)
				}
				h = base * amp
				for _, t := range w.Towns {
					d := math.Hypot(float64(x)-t.X, float64(y)-t.Y)
					h *= smoothstep((d - t.R - 2) / 14)
				}
				if castle != nil {
					cx, cy := float64(castle.X)+float64(castle.W)/2, float64(castle.Y)+float64(castle.H)/2
					d := math.Hypot(float64(x)-cx, float64(y)-cy)
					h = math.Max(h, 3.5*(1-smoothstep((d-float64(castle.W)/2)/7)))
				}
			}
			if w.Underground(float64(x), float64(y)) {
				h = -10
			}
			w.Heights[y*(w.Size+1)+x] = math.Round(h*100) / 100
		}
	}
}

// ClearLine prevents melee and interactions through walls and buildings.
func (w *World) ClearLine(ax, ay, bx, by float64) bool {
	steps := int(math.Ceil(math.Hypot(bx-ax, by-ay) * 8))
	for i := 1; i < steps; i++ {
		t := float64(i) / float64(steps)
		x, y := int(math.Floor(ax+(bx-ax)*t)), int(math.Floor(ay+(by-ay)*t))
		if !w.InBounds(x, y) || Solid(w.Terrain[w.Index(x, y)]) {
			return false
		}
	}
	return true
}
