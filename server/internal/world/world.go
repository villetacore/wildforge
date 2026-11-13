// Package world generates and queries the tile map: terrain plus gatherable
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
)

var terrainNames = map[string]Terrain{
	"grass": Grass, "water": Water, "sand": Sand, "forest": Forest, "road": Road, "rocky": Rocky,
}

type World struct {
	Heights []float64
	Portals []PortalLink
	Size    int
	Terrain []Terrain
	Nodes   []uint8 // resource kind per tile, 1-based index into content resources, 0 = none
	Charges []int   // remaining gathers per node; 0 = depleted
	SpawnX  float64
	SpawnY  float64
}

// Generate builds an island from noise and scatters resource nodes according
// to the spawn rules in the content pack.
func Generate(seed uint32, pack *content.Pack) *World {
	n := pack.World.Size
	w := &World{Size: n, Terrain: make([]Terrain, n*n), Nodes: make([]uint8, n*n), Charges: make([]int, n*n)}
	c := float64(n) / 2
	scale := float64(n) / 256 // keep feature sizes similar for other map sizes
	for y := 0; y < n; y++ {
		for x := 0; x < n; x++ {
			fx, fy := float64(x)/scale, float64(y)/scale
			e := FBM(fx/48, fy/48, seed)
			dx, dy := (float64(x)-c)/c, (float64(y)-c)/c
			d := math.Sqrt(dx*dx + dy*dy)
			e = e + 0.22 - math.Pow(d, 2.8)*0.6 // sink the borders: an island
			moist := FBM(fx/32+500, fy/32+500, seed^0xBEEF)
			rock := FBM(fx/16+900, fy/16+900, seed^0xC0DE)
			t := Grass
			switch {
			case e < 0.32:
				t = Water
			case e < 0.36:
				t = Sand
			case math.Hypot(float64(x)-c, float64(y)-c) < pack.World.TownRadius:
				t = Grass
			case rock > 0.6 && e > 0.48:
				t = Rocky
			case moist > 0.56:
				t = Forest
			}
			w.Terrain[y*n+x] = t
		}
	}
	// Roads cross the island through the town.
	for i := 0; i < n; i++ {
		for off := 0; off < 2; off++ {
			for _, p := range [][2]int{{i, n/2 + off}, {n/2 + off, i}} {
				if idx := p[1]*n + p[0]; w.Terrain[idx] != Water {
					w.Terrain[idx] = Road
				}
			}
		}
	}
	// Resource nodes, rarest first (content order decides priority).
	for y := 1; y < n-1; y++ {
		for x := 1; x < n-1; x++ {
			idx := y*n + x
			t := w.Terrain[idx]
			if t == Water || t == Road || math.Hypot(float64(x)-c, float64(y)-c) < pack.World.TownRadius {
				continue
			}
			dist := math.Hypot(float64(x)-c, float64(y)-c) / c
		resources:
			for ri, r := range pack.Resources {
				if dist < r.MinDistance {
					continue
				}
				roll := Hash(int32(x), int32(y), seed^uint32(0x51ED+ri*7919))
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
	w.SpawnX, w.SpawnY = c+0.5, c+0.5
	w.generateDungeons(seed)
	w.generateHeights(seed)
	return w
}

func (w *World) Index(x, y int) int { return y*w.Size + x }

func (w *World) InBounds(x, y int) bool { return x >= 0 && y >= 0 && x < w.Size && y < w.Size }

// Blocked reports whether a tile cannot be walked on.
func (w *World) Blocked(x, y int) bool {
	if !w.InBounds(x, y) {
		return true
	}
	i := w.Index(x, y)
	return (w.Terrain[i] == Water || w.Terrain[i] == Wall) || (w.Nodes[i] != 0 && w.Charges[i] > 0)
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
