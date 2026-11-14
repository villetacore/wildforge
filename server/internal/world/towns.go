package world

import (
	"fmt"
	"math"
)

// Structure is a building or prop placed in a town. Solid structures block
// their footprint (terrain Building). The client builds the 3D model from
// Kind/Style/Floors and colors.
type Structure struct {
	X      int    `json:"x"`
	Y      int    `json:"y"`
	W      int    `json:"w"`
	H      int    `json:"h"`
	Kind   string `json:"kind"`
	Style  string `json:"style"`
	Floors int    `json:"floors"`
	Wall   string `json:"wall"`
	Roof   string `json:"roof"`
	Name   string `json:"name,omitempty"`
	Solid  bool   `json:"solid"`
	Door   int    `json:"door"` // side with the entrance: 0 north (-y), 1 east, 2 south, 3 west
}

// NPCSpot is where a town NPC stands.
type NPCSpot struct {
	Role string  `json:"role"`
	X    float64 `json:"x"`
	Y    float64 `json:"y"`
	Town int     `json:"town"`
	Name string  `json:"name,omitempty"` // fixed name (e.g. a named courtesan)
	Tier int     `json:"tier,omitempty"` // role-specific variant
}

var pastel = []string{"#efe3c8", "#e8c9a0", "#f0d8d0", "#d8e0c8", "#f2ecd8", "#e8d0a8", "#d8c8b8", "#f0e0b0", "#c8d8e0", "#e0c0b0"}
var redRoofs = []string{"#b0482a", "#a03c22", "#c05a30", "#984028", "#b85a38", "#8a3a24"}

type styleColors struct {
	walls, roofs []string
	floors       [2]int
}

var townStyles = map[string]styleColors{
	"capital":   {pastel, redRoofs, [2]int{2, 4}},
	"timber":    {[]string{"#e8dcc0", "#d8c8a0", "#c8b890"}, []string{"#5a4a3a", "#4a5a3a", "#6a4a30"}, [2]int{1, 2}},
	"nordic":    {[]string{"#8a7a6a", "#7a6a5a", "#9a8a78"}, []string{"#f0f4f8", "#e0e8f0"}, [2]int{1, 2}},
	"basalt":    {[]string{"#4a4440", "#3a3634", "#5a504a"}, []string{"#8a2a1a", "#6a2a20", "#3a3030"}, [2]int{1, 3}},
	"stilts":    {[]string{"#7a6a4a", "#6a5a3a", "#8a7a5a"}, []string{"#a09a5a", "#8a8a4a", "#b0a060"}, [2]int{1, 1}},
	"sandstone": {[]string{"#e0c890", "#d8b878", "#e8d0a0"}, []string{"#d0b070", "#c8a060", "#e0c080"}, [2]int{1, 2}},
}

func pick(list []string, x, y int, salt uint32) string {
	return list[int(Hash(int32(x), int32(y), salt)*float64(len(list)-1)+0.5)]
}

func (w *World) set(x, y int, t Terrain) {
	if w.InBounds(x, y) {
		i := w.Index(x, y)
		w.Terrain[i], w.Nodes[i], w.Charges[i] = t, 0, 0
	}
}

func (w *World) at(x, y int) Terrain {
	if !w.InBounds(x, y) {
		return Water
	}
	return w.Terrain[w.Index(x, y)]
}

// addStructure registers a structure and blocks its footprint if solid.
// It refuses (returns false) when the footprint is not entirely free ground.
func (w *World) addStructure(s Structure, free func(x, y int) bool) bool {
	for y := s.Y; y < s.Y+s.H; y++ {
		for x := s.X; x < s.X+s.W; x++ {
			if !free(x, y) {
				return false
			}
		}
	}
	if s.Solid {
		for y := s.Y; y < s.Y+s.H; y++ {
			for x := s.X; x < s.X+s.W; x++ {
				w.set(x, y, Building)
			}
		}
	}
	w.Structures = append(w.Structures, s)
	return true
}

// doorSpot returns the walkable tile in front of a structure's door.
func doorSpot(s Structure) (float64, float64) {
	cx, cy := float64(s.X)+float64(s.W)/2, float64(s.Y)+float64(s.H)/2
	switch s.Door {
	case 0:
		return cx, float64(s.Y) - 0.6
	case 1:
		return float64(s.X+s.W) + 0.6, cy
	case 2:
		return cx, float64(s.Y+s.H) + 0.6
	default:
		return float64(s.X) - 0.6, cy
	}
}

// facing picks the door side that points toward (tx, ty).
func facing(x, y, w, h int, tx, ty float64) int {
	dx := tx - (float64(x) + float64(w)/2)
	dy := ty - (float64(y) + float64(h)/2)
	if math.Abs(dx) > math.Abs(dy) {
		if dx > 0 {
			return 1
		}
		return 3
	}
	if dy > 0 {
		return 2
	}
	return 0
}

func (w *World) spot(role string, x, y float64, town int) {
	w.NPCSpots = append(w.NPCSpots, NPCSpot{Role: role, X: x, Y: y, Town: town})
}

// settleSpots moves every NPC spot onto the nearest walkable tile.
func (w *World) settleSpots() {
	taken := map[int]bool{}
	for i := range w.NPCSpots {
		s := &w.NPCSpots[i]
		x0, y0 := s.X, s.Y
	search:
		for r := 0.0; r < 6; r += 0.5 {
			for a := 0.0; a < 2*math.Pi; a += math.Pi / 8 {
				x, y := x0+math.Cos(a)*r, y0+math.Sin(a)*r
				if w.InBounds(int(x), int(y)) && w.Free(x, y) && !taken[w.Index(int(x), int(y))] {
					s.X, s.Y = x, y
					taken[w.Index(int(x), int(y))] = true
					break search
				}
				if r == 0 {
					break
				}
			}
		}
	}
}

func (w *World) buildTowns() {
	for i, t := range w.Towns {
		if t.Style == "capital" {
			w.buildCapital(i)
		} else {
			w.buildTown(i)
		}
	}
}

// buildCapital lays out Velhrad, a city on both banks of the river: the
// castle hill with palace and cathedral on the west bank, the Old Town with
// its square, astronomical clock and twin-spired church on the east bank,
// and the King's Bridge with its statues between them.
func (w *World) buildCapital(ti int) {
	t := w.Towns[ti]
	c := int(t.X)
	R := int(t.R)
	style := townStyles["capital"]
	reserved := map[int]bool{}
	inCity := func(x, y int) bool { return math.Hypot(float64(x-c)+.5, float64(y-c)+.5) < float64(R)-0.5 }

	// Ground: everything dry inside the walls becomes cobble.
	for y := c - R - 2; y <= c+R+2; y++ {
		for x := c - R - 2; x <= c+R+2; x++ {
			if !w.InBounds(x, y) {
				continue
			}
			d := math.Hypot(float64(x-c)+.5, float64(y-c)+.5)
			cur := w.at(x, y)
			switch {
			case d < float64(R)-0.5:
				if cur != Water && cur != Bridge {
					w.set(x, y, Cobble)
				}
				if cur == Road || cur == Bridge {
					reserved[w.Index(x, y)] = true
				}
			case d < float64(R)+1:
				if cur == Road || cur == Bridge {
					reserved[w.Index(x, y)] = true // gate
					if cur == Road {
						w.set(x, y, Cobble)
					}
				} else if cur != Water {
					w.set(x, y, CityWall)
				}
			case d < float64(R)+3 && cur != Water && cur != Bridge && cur != Road:
				w.set(x, y, Grass)
			}
		}
	}
	// Gate guards on the cross streets.
	for _, g := range [][2]float64{{float64(c - R - 2), float64(c) + 2.5}, {float64(c + R + 2), float64(c) - 0.5}, {float64(c) + 2.5, float64(c - R - 2)}, {float64(c) - 0.5, float64(c + R + 2)}} {
		w.spot("guard", g[0], g[1], ti)
	}

	// The King's Bridge: three tiles wide, statues on the parapets.
	by := c - 1
	for x := c; x < c+R; x++ {
		for y := by; y <= by+3; y++ {
			if w.at(x, y) == Water {
				w.set(x, y, Bridge)
				reserved[w.Index(x, y)] = true
			}
		}
	}
	// A second, narrower bridge further north.
	for x := c; x < c+R; x++ {
		for y := c - 17; y <= c-16; y++ {
			if w.at(x, y) == Water {
				w.set(x, y, Bridge)
				reserved[w.Index(x, y)] = true
			}
		}
	}
	bx0, bx1 := -1, -1
	for x := c; x < c+R; x++ {
		if w.at(x, by) == Bridge {
			if bx0 < 0 {
				bx0 = x
			}
			bx1 = x
		}
	}
	if bx0 >= 0 {
		w.Structures = append(w.Structures, Structure{X: bx0, Y: by, W: bx1 - bx0 + 1, H: 4, Kind: "kings_bridge", Style: "capital", Name: "King's Bridge", Wall: "#b8ac98"})
		w.spot("guard", float64(bx0)-0.8, float64(by)-0.3, ti)
		w.spot("guard", float64(bx1)+1.8, float64(by)+4.3, ti)
	}

	free := func(x, y int) bool {
		return inCity(x, y) && w.at(x, y) == Cobble && !reserved[w.Index(x, y)]
	}
	reserve := func(x0, y0, ww, hh, margin int) {
		for y := y0 - margin; y < y0+hh+margin; y++ {
			for x := x0 - margin; x < x0+ww+margin; x++ {
				if w.InBounds(x, y) {
					reserved[w.Index(x, y)] = true
				}
			}
		}
	}
	special := func(s Structure, role ...string) (Structure, bool) {
		if s.Wall == "" {
			s.Wall = pick(style.walls, s.X, s.Y, 11)
		}
		if s.Roof == "" {
			s.Roof = pick(style.roofs, s.X, s.Y, 12)
		}
		s.Style = "capital"
		s.Solid = true
		// Nudge the building around its planned spot until it fits.
		placed := false
		x0, y0 := s.X, s.Y
	search:
		for r := 0; r <= 7; r++ {
			for dy := -r; dy <= r; dy++ {
				for dx := -r; dx <= r; dx++ {
					if max(abs(dx), abs(dy)) != r {
						continue
					}
					s.X, s.Y = x0+dx, y0+dy
					if w.addStructure(s, free) {
						placed = true
						break search
					}
				}
			}
		}
		if !placed {
			return s, false
		}
		reserve(s.X, s.Y, s.W, s.H, 1)
		dx, dy := doorSpot(s)
		for i, r := range role {
			off := float64(i) - float64(len(role)-1)/2
			if s.Door == 0 || s.Door == 2 {
				w.spot(r, dx+off*1.3, dy, ti)
			} else {
				w.spot(r, dx, dy+off*1.3, ti)
			}
		}
		return s, true
	}

	// Old Town Square (east bank).
	sx0, sy0, sw, sh := c+14, c-5, 13, 11
	reserve(sx0, sy0, sw, sh, 0)
	w.Structures = append(w.Structures, Structure{X: sx0, Y: sy0, W: sw, H: sh, Kind: "square", Style: "capital", Name: "Old Town Square"})
	w.addStructure(Structure{X: sx0 + 6, Y: sy0 + 5, W: 1, H: 1, Kind: "monument", Style: "capital", Solid: true, Name: "Statue of the Founder", Wall: "#5a6a5a"}, func(int, int) bool { return true })
	w.addStructure(Structure{X: sx0 + 2, Y: sy0 + 7, W: 2, H: 1, Kind: "stall", Style: "capital", Solid: true, Roof: "#c03a3a", Wall: "#8a6a4a", Door: 0}, func(int, int) bool { return true })
	w.addStructure(Structure{X: sx0 + 9, Y: sy0 + 7, W: 2, H: 1, Kind: "stall", Style: "capital", Solid: true, Roof: "#3a6ac0", Wall: "#8a6a4a", Door: 0}, func(int, int) bool { return true })
	w.spot("merchant", float64(sx0)+3, float64(sy0)+6.4, ti)
	w.spot("citizen", float64(sx0)+10, float64(sy0)+6.4, ti)
	w.spot("citizen", float64(sx0)+4.5, float64(sy0)+2.5, ti)
	for _, p := range [][2]int{{sx0 + 1, sy0 + 1}, {sx0 + sw - 2, sy0 + 1}, {sx0 + 1, sy0 + sh - 2}, {sx0 + sw - 2, sy0 + sh - 2}} {
		w.Structures = append(w.Structures, Structure{X: p[0], Y: p[1], W: 1, H: 1, Kind: "lamp", Style: "capital"})
	}

	special(Structure{X: sx0 + 1, Y: sy0 - 5, W: 9, H: 4, Kind: "townhall", Floors: 3, Name: "Old Town Hall", Wall: "#e8dcc0", Roof: "#8a3a24", Door: 2}, "citizen")
	special(Structure{X: sx0 + sw + 1, Y: sy0 + 1, W: 5, H: 8, Kind: "church", Floors: 4, Name: "Church of Our Lady before Velhrad", Wall: "#d8d0c0", Roof: "#3a3a40", Door: 3}, "healer")
	tav, ok := special(Structure{X: sx0, Y: sy0 + sh + 1, W: 7, H: 5, Kind: "tavern", Floors: 2, Name: "The Drunken Griffin", Wall: "#e8c9a0", Roof: "#a03c22", Door: 0})
	special(Structure{X: sx0 + 8, Y: sy0 + sh + 1, W: 5, H: 5, Kind: "smithy", Floors: 1, Name: "Hammer & Anvil", Wall: "#a09080", Roof: "#5a4a40", Door: 0}, "blacksmith")
	// The tavern yard: tables in the square, innkeeper, board and company.
	if ok {
		ty := float64(tav.Y) - 0.6
		w.spot("innkeeper", float64(tav.X)+2, ty, ti)
		w.spot("board", float64(tav.X)+3.5, ty-0.2, ti)
		w.NPCSpots = append(w.NPCSpots,
			NPCSpot{Role: "courtesan", X: float64(tav.X) + 5.2, Y: ty, Town: ti, Name: "Rosa", Tier: 0},
			NPCSpot{Role: "courtesan", X: float64(tav.X) + 6.4, Y: ty - 0.3, Town: ti, Name: "Marlene", Tier: 1},
			NPCSpot{Role: "courtesan", X: float64(tav.X) + 0.6, Y: ty - 0.3, Town: ti, Name: "Lady Vivienne", Tier: 2},
			NPCSpot{Role: "exotic", X: float64(tav.X) - 0.4, Y: ty - 1.4, Town: ti},
		)
		for _, p := range [][2]int{{tav.X + 1, tav.Y - 3}, {tav.X + 4, tav.Y - 3}} {
			w.Structures = append(w.Structures, Structure{X: p[0], Y: p[1], W: 1, H: 1, Kind: "table", Style: "capital"})
		}
	}
	special(Structure{X: sx0 + sw - 4, Y: sy0 + sh + 7, W: 6, H: 3, Kind: "stable", Floors: 1, Name: "Royal Stables", Wall: "#a07a50", Roof: "#6a4a30", Door: 0}, "stablemaster")
	special(Structure{X: sx0 + sw + 1, Y: sy0 - 6, W: 3, H: 3, Kind: "shop", Floors: 2, Name: "Barber Surgeon", Door: 3}, "barber")

	// Castle hill on the west bank: walls, palace, cathedral.
	kx, ky, kr := c-17, c-11, 10
	w.Structures = append(w.Structures, Structure{X: kx - kr, Y: ky - kr, W: 2 * kr, H: 2 * kr, Kind: "castle_hill", Style: "capital", Name: "Velhrad Castle"})
	for y := ky - kr - 1; y <= ky+kr+1; y++ {
		for x := kx - kr - 1; x <= kx+kr+1; x++ {
			d := math.Hypot(float64(x-kx)+.5, float64(y-ky)+.5)
			if !inCity(x, y) || w.at(x, y) == Water {
				continue
			}
			reserved[w.Index(x, y)] = true
			gate := (y == ky || y == ky+1) && x > kx
			if d >= float64(kr)-0.7 && d < float64(kr)+0.5 && !gate {
				w.set(x, y, CityWall)
			}
		}
	}
	castleFree := func(x, y int) bool {
		return math.Hypot(float64(x-kx)+.5, float64(y-ky)+.5) < float64(kr)-1.2 && w.at(x, y) == Cobble
	}
	palace := Structure{X: kx - 5, Y: ky - 6, W: 8, H: 4, Kind: "palace", Style: "capital", Floors: 4, Name: "Royal Palace", Wall: "#ece4d0", Roof: "#5a7a6a", Solid: true, Door: 2}
	if w.addStructure(palace, castleFree) {
		dx, dy := doorSpot(palace)
		w.spot("king", dx, dy+0.4, ti)
		w.spot("quartermaster", dx+2.5, dy, ti)
		w.spot("guard", dx-2, dy, ti)
	}
	w.addStructure(Structure{X: kx - 4, Y: ky + 2, W: 10, H: 4, Kind: "cathedral", Style: "capital", Floors: 5, Name: "Cathedral of St. Vojtech", Wall: "#c8c0b0", Roof: "#4a6a5a", Solid: true, Door: 1}, castleFree)
	w.spot("guard", float64(kx+kr)+1.2, float64(ky)+1, ti)

	// Arcane Collegium: a round tower on the west bank, south of the castle.
	special(Structure{X: c - 15, Y: c + 8, W: 5, H: 5, Kind: "magetower", Floors: 6, Name: "Arcane Collegium", Wall: "#b8b0c8", Roof: "#3a3a8a", Door: 1}, "enchanter", "trainer")
	special(Structure{X: c - 20, Y: c + 17, W: 6, H: 4, Kind: "shop", Floors: 2, Name: "Herbalist", Door: 0}, "healer")

	// Fill the rest with dense blocks of houses separated by narrow lanes.
	for by := c - R; by < c+R; by += 6 {
		for bx := c - R; bx < c+R; bx += 6 {
			x0, y0 := bx+1, by+1
			ok := true
			for y := y0; y < y0+5 && ok; y++ {
				for x := x0; x < x0+5 && ok; x++ {
					ok = free(x, y)
				}
			}
			if !ok {
				// Try a smaller house in a partial block.
				x0, y0 = bx+2, by+2
				ok = true
				for y := y0; y < y0+3 && ok; y++ {
					for x := x0; x < x0+3 && ok; x++ {
						ok = free(x, y)
					}
				}
				if ok {
					w.house(x0, y0, 3, 3, "capital", c, c)
				}
				continue
			}
			if Hash(int32(bx), int32(by), w.seed+5) < 0.5 {
				w.house(x0, y0, 5, 2, "capital", c, c)
				w.house(x0, y0+3, 5, 2, "capital", c, c)
			} else {
				w.house(x0, y0, 2, 5, "capital", c, c)
				w.house(x0+3, y0, 2, 5, "capital", c, c)
			}
		}
	}
	// Street lamps along the main streets.
	for d := -R + 4; d < R-3; d += 7 {
		w.Structures = append(w.Structures,
			Structure{X: c + d, Y: c - 1, W: 1, H: 1, Kind: "lamp", Style: "capital"},
			Structure{X: c - 1, Y: c + d, W: 1, H: 1, Kind: "lamp", Style: "capital"})
	}
	for i := 0; i < 6; i++ {
		a := float64(i) * 1.1
		x, y := float64(c)+math.Cos(a)*float64(R-8), float64(c)+math.Sin(a)*float64(R-8)
		if w.Free(x, y) {
			w.spot("citizen", x, y, ti)
		}
	}
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func (w *World) house(x, y, ww, hh int, style string, cx, cy int) {
	st := townStyles[style]
	floors := st.floors[0] + int(Hash(int32(x), int32(y), w.seed+9)*float64(st.floors[1]-st.floors[0]+1)*0.999)
	w.Structures = append(w.Structures, Structure{
		X: x, Y: y, W: ww, H: hh, Kind: "house", Style: style, Floors: floors, Solid: true,
		Wall: pick(st.walls, x, y, 3), Roof: pick(st.roofs, x, y, 4), Door: facing(x, y, ww, hh, float64(cx), float64(cy)),
	})
	for yy := y; yy < y+hh; yy++ {
		for xx := x; xx < x+ww; xx++ {
			w.set(xx, yy, Building)
		}
	}
}

// buildTown lays out a smaller settlement: a cobbled plaza with a well and
// a ring of buildings around it in the zone's architectural style.
func (w *World) buildTown(ti int) {
	t := w.Towns[ti]
	cx, cy := int(t.X), int(t.Y)
	R := int(t.R)
	for y := cy - R - 1; y <= cy+R+1; y++ {
		for x := cx - R - 1; x <= cx+R+1; x++ {
			d := math.Hypot(float64(x-cx)+.5, float64(y-cy)+.5)
			cur := w.at(x, y)
			if !w.InBounds(x, y) || d > float64(R)+1 {
				continue
			}
			switch {
			case d < 5.5:
				if cur != Bridge {
					w.set(x, y, Cobble)
				}
			case cur == Water || cur == Lava:
				if d < float64(R)-1 {
					w.set(x, y, Bridge) // boardwalks over pools in the mire
				}
			case cur == Road || cur == Bridge:
			default:
				ground := Grass
				switch t.Style {
				case "nordic":
					ground = Snow
				case "basalt":
					ground = Ash
				case "stilts":
					ground = Swamp
				case "sandstone":
					ground = Sand
				}
				w.set(x, y, ground)
			}
		}
	}
	w.Structures = append(w.Structures, Structure{X: cx - 5, Y: cy - 5, W: 11, H: 11, Kind: "plaza", Style: t.Style, Name: t.Name})
	w.addStructure(Structure{X: cx, Y: cy, W: 1, H: 1, Kind: "well", Style: t.Style, Solid: true, Wall: "#8a8a8a"}, func(int, int) bool { return true })

	roles := [][]string{
		{"innkeeper", "board"},
		{"blacksmith"},
		{"merchant"},
		{"healer"},
		{"quartermaster"},
		{"stablemaster"},
		{},
		{},
	}
	kinds := []string{"tavern", "smithy", "shop", "temple", "barracks", "stable", "house", "house"}
	names := []string{t.Name + " Inn", "Smithy", "Trading Post", "Shrine", "Lodge", "Stables", "", ""}
	st := townStyles[t.Style]
	placed := 0
	for i := 0; i < 12 && placed < len(kinds); i++ {
		a := float64(i)/12*2*math.Pi + Hash(int32(ti), 3, w.seed)*0.5
		r := 8.5
		bw, bh := 5, 4
		if kinds[placed] == "house" {
			bw, bh = 4, 3
		}
		x := cx + int(math.Round(math.Cos(a)*r)) - bw/2
		y := cy + int(math.Round(math.Sin(a)*r)) - bh/2
		s := Structure{
			X: x, Y: y, W: bw, H: bh, Kind: kinds[placed], Style: t.Style, Solid: true, Name: names[placed],
			Floors: st.floors[0] + placed%max(1, st.floors[1]-st.floors[0]+1),
			Wall:   pick(st.walls, x, y, 3), Roof: pick(st.roofs, x, y, 4), Door: facing(x, y, bw, bh, t.X, t.Y),
		}
		ok := w.addStructure(s, func(x, y int) bool {
			c := w.at(x, y)
			return c != Water && c != Bridge && c != Road && c != Building && math.Hypot(float64(x-cx)+.5, float64(y-cy)+.5) > 5.5 &&
				math.Hypot(float64(x-cx)+.5, float64(y-cy)+.5) < float64(R)
		})
		if !ok {
			continue
		}
		dx, dy := doorSpot(s)
		for j, role := range roles[placed] {
			off := (float64(j) - float64(len(roles[placed])-1)/2) * 1.3
			if s.Door == 0 || s.Door == 2 {
				w.spot(role, dx+off, dy, ti)
			} else {
				w.spot(role, dx, dy+off, ti)
			}
		}
		placed++
	}
	w.spot("guard", t.X-2.5, t.Y-2.5, ti)
	w.spot("guard", t.X+2.5, t.Y+2.5, ti)
	w.spot("citizen", t.X+2, t.Y-2.2, ti)
	if t.Style == "stilts" {
		w.NPCSpots = append(w.NPCSpots, NPCSpot{Role: "courtesan", X: t.X - 2.2, Y: t.Y + 1.5, Town: ti, Name: "Swamp Nettie", Tier: 0})
	}
	if t.Style == "sandstone" {
		w.NPCSpots = append(w.NPCSpots, NPCSpot{Role: "courtesan", X: t.X - 2.2, Y: t.Y + 1.5, Town: ti, Name: "Desert Rose", Tier: 1})
	}
	if t.Style == "basalt" {
		w.spot("enchanter", t.X+2.4, t.Y-1.2, ti)
		w.spot("trainer", t.X-2.4, t.Y+1.2, ti)
	}
	for i := 0; i < 4; i++ {
		a := float64(i)*math.Pi/2 + 0.4
		w.Structures = append(w.Structures, Structure{X: cx + int(math.Cos(a)*4.5), Y: cy + int(math.Sin(a)*4.5), W: 1, H: 1, Kind: "lamp", Style: t.Style})
	}
}

// String is used in tests and logs.
func (s Structure) String() string { return fmt.Sprintf("%s@%d,%d", s.Kind, s.X, s.Y) }
