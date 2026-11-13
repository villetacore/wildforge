package world

import "math"

const (
	Wall Terrain = 6 + iota
	DungeonFloor
	Portal
	Chest
)

type PortalLink struct {
	X   float64 `json:"x"`
	Y   float64 `json:"y"`
	ToX float64 `json:"to_x"`
	ToY float64 `json:"to_y"`
}

// Dungeons occupy isolated, lowered areas of the shared world. Their enclosing
// walls make portals the only entrance; every room is connected by a corridor.
func (w *World) generateDungeons(seed uint32) {
	if w.Size < 96 {
		return
	}
	for d := 0; d < 2; d++ {
		ox, oy := 2+d*(w.Size-48), 2
		for y := oy; y < oy+44; y++ {
			for x := ox; x < ox+44; x++ {
				i := w.Index(x, y)
				w.Terrain[i], w.Nodes[i], w.Charges[i] = Wall, 0, 0
			}
		}
		carve := func(x, y int) { w.Terrain[w.Index(x, y)] = DungeonFloor }
		px, py := ox+7, oy+7
		for room := 0; room < 6; room++ {
			x := ox + 7 + int(Hash(int32(room), int32(d), seed+91)*29)
			y := oy + 7 + int(Hash(int32(room), int32(d), seed+193)*29)
			if room == 0 {
				x, y = px, py
			}
			for yy := y - 3; yy <= y+3; yy++ {
				for xx := x - 3; xx <= x+3; xx++ {
					carve(xx, yy)
				}
			}
			for xx := min(px, x); xx <= max(px, x); xx++ {
				for off := -1; off <= 1; off++ {
					carve(xx, py+off)
				}
			}
			for yy := min(py, y); yy <= max(py, y); yy++ {
				for off := -1; off <= 1; off++ {
					carve(x+off, yy)
				}
			}
			px, py = x, y
		}
		// Choose scattered floor cells after carving, preserving every connection.
		count := 0
		var caches [][2]int
		for y := oy + 10; y < oy+40 && count < 5; y++ {
			for x := ox + 10; x < ox+40 && count < 5; x++ {
				i := w.Index(x, y)
				spaced := true
				for _, c := range caches {
					if math.Hypot(float64(x-c[0]), float64(y-c[1])) < 7 {
						spaced = false
					}
				}
				if spaced && w.Terrain[i] == DungeonFloor && Hash(int32(x), int32(y), seed+82) > .93 {
					caches = append(caches, [2]int{x, y})
					w.Terrain[i] = Chest
					count++
				}
			}
		}
		ex, ey := w.Size/2+5+d*5, w.Size/2
		for y := ey - 2; y <= ey+2; y++ {
			for x := ex - 2; x <= ex+2; x++ {
				i := w.Index(x, y)
				w.Terrain[i], w.Nodes[i], w.Charges[i] = Road, 0, 0
			}
		}
		dx, dy := ox+7, oy+7
		w.Terrain[w.Index(ex, ey)] = Portal
		w.Terrain[w.Index(dx, dy)] = Portal
		w.Portals = append(w.Portals, PortalLink{float64(ex) + .5, float64(ey) + .5, float64(dx) + .5, float64(dy) + .5},
			PortalLink{float64(dx) + .5, float64(dy) + .5, float64(ex) + .5, float64(ey) + .5})
	}
}

func (w *World) Underground(x, y float64) bool {
	return w.Size >= 96 && y < 46 && (x < 46 || x >= float64(w.Size-46))
}

func (w *World) generateHeights(seed uint32) {
	w.Heights = make([]float64, (w.Size+1)*(w.Size+1))
	for y := 0; y <= w.Size; y++ {
		for x := 0; x <= w.Size; x++ {
			h := 0.0
			wet := false
			for dy := -1; dy <= 0; dy++ {
				for dx := -1; dx <= 0; dx++ {
					if !w.InBounds(x+dx, y+dy) || w.Terrain[w.Index(x+dx, y+dy)] == Water {
						wet = true
					}
				}
			}
			if !wet {
				d := math.Hypot(float64(x)-w.SpawnX, float64(y)-w.SpawnY)
				h = math.Max(0, FBM(float64(x)/35, float64(y)/35, seed+812)-.25) * 12 * math.Min(1, d/18)
			}
			if w.Underground(float64(x), float64(y)) {
				h = -10
			}
			w.Heights[y*(w.Size+1)+x] = math.Round(h*100) / 100
		}
	}
}

// ClearLine prevents melee and interactions through dungeon walls.
func (w *World) ClearLine(ax, ay, bx, by float64) bool {
	steps := int(math.Ceil(math.Hypot(bx-ax, by-ay) * 8))
	for i := 1; i < steps; i++ {
		t := float64(i) / float64(steps)
		x, y := int(math.Floor(ax+(bx-ax)*t)), int(math.Floor(ay+(by-ay)*t))
		if !w.InBounds(x, y) || w.Terrain[w.Index(x, y)] == Wall {
			return false
		}
	}
	return true
}
