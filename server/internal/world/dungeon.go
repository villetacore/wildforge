package world

import (
	"math"
	"sort"
)

const DungeonSize = 56

type PortalLink struct {
	X   float64 `json:"x"`
	Y   float64 `json:"y"`
	ToX float64 `json:"to_x"`
	ToY float64 `json:"to_y"`
}

type Room struct{ X, Y, W, H int }

func (r Room) Center() (int, int) { return r.X + r.W/2, r.Y + r.H/2 }

// Dungeon is an underground area in a reserved square of the map. Its layout
// is regenerated from a new seed every time it resets, so each run differs.
type Dungeon struct {
	Index    int      `json:"index"`
	Zone     int      `json:"zone"`
	Name     string   `json:"name"`
	Theme    string   `json:"theme"`
	X0       int      `json:"x"`
	Y0       int      `json:"y"`
	Size     int      `json:"size"`
	Gen      int      `json:"gen"`
	SurfaceX float64  `json:"surface_x"`
	SurfaceY float64  `json:"surface_y"`
	Rooms    []Room   `json:"-"`
	Entry    int      `json:"-"`
	Boss     int      `json:"-"`
	Spawns   [][3]int `json:"-"` // x, y, kind (0 guard, 1 boss)
}

// Contains reports whether a position is inside the dungeon square.
func (d *Dungeon) Contains(x, y float64) bool {
	return x >= float64(d.X0) && y >= float64(d.Y0) && x < float64(d.X0+d.Size) && y < float64(d.Y0+d.Size)
}

func (w *World) dungeonSlots() [][2]int {
	n, s := w.Size, DungeonSize
	return [][2]int{{2, 2}, {n - s - 2, 2}, {2, n - s - 2}, {n - s - 2, n - s - 2}, {n/2 - s/2 - 60, 2}, {n/2 - s/2 + 60, n - s - 2}}
}

// generateDungeons digs one dungeon per zone and an entrance near its town.
func (w *World) generateDungeons(seed uint32) {
	if w.Size < 2*DungeonSize+8 {
		return
	}
	slots := w.dungeonSlots()
	for zi, z := range w.pack.Zones {
		if zi >= len(slots) {
			break
		}
		d := &Dungeon{Index: zi, Zone: zi, Name: z.Dungeon.Name, Theme: z.Dungeon.Theme, X0: slots[zi][0], Y0: slots[zi][1], Size: DungeonSize}
		w.Dungeons = append(w.Dungeons, d)
		// Surface entrance: a clearing 16-24 tiles from the town, on dry land.
		t := w.Towns[zi]
		ex, ey := t.X+t.R+6, t.Y
		for tries := 0; tries < 80; tries++ {
			a := Hash(int32(zi), int32(tries), seed+404) * 2 * math.Pi
			r := t.R + 6 + float64(tries%8)
			x, y := t.X+math.Cos(a)*r, t.Y+math.Sin(a)*r
			if w.landScore(int(x), int(y), 3) == 1 && !w.Underground(x, y) {
				ex, ey = x, y
				break
			}
		}
		ix, iy := int(ex), int(ey)
		for y := iy - 2; y <= iy+2; y++ {
			for x := ix - 2; x <= ix+2; x++ {
				if w.InBounds(x, y) {
					w.set(x, y, Road)
				}
			}
		}
		w.set(ix, iy, Portal)
		d.SurfaceX, d.SurfaceY = float64(ix)+.5, float64(iy)+.5
		w.Portals = append(w.Portals, PortalLink{X: d.SurfaceX, Y: d.SurfaceY}, PortalLink{ToX: d.SurfaceX, ToY: d.SurfaceY})
		w.road(t.X, t.Y, d.SurfaceX, d.SurfaceY, 1)
		w.set(ix, iy, Portal)
		w.RegenDungeon(zi, seed+uint32(zi)*7919)
	}
}

// RegenDungeon carves a fresh layout: rooms connected by a minimum spanning
// tree of corridors plus a few loops, pillars in large halls, caches in rooms
// and a boss chamber in the room farthest from the entrance.
func (w *World) RegenDungeon(i int, seed uint32) {
	d := w.Dungeons[i]
	d.Gen++
	rng := func(a, b int32) float64 { return Hash(a, b, seed+uint32(d.Gen)*104729) }
	ox, oy, S := d.X0, d.Y0, d.Size
	for y := oy; y < oy+S; y++ {
		for x := ox; x < ox+S; x++ {
			w.set(x, y, Wall)
		}
	}
	// Rooms.
	d.Rooms = d.Rooms[:0]
	for try := int32(0); try < 200 && len(d.Rooms) < 11; try++ {
		rw := 5 + int(rng(try, 1)*8)
		rh := 5 + int(rng(try, 2)*8)
		r := Room{X: ox + 3 + int(rng(try, 3)*float64(S-6-rw)), Y: oy + 3 + int(rng(try, 4)*float64(S-6-rh)), W: rw, H: rh}
		ok := true
		for _, o := range d.Rooms {
			if r.X < o.X+o.W+2 && o.X < r.X+r.W+2 && r.Y < o.Y+o.H+2 && o.Y < r.Y+r.H+2 {
				ok = false
				break
			}
		}
		if ok {
			d.Rooms = append(d.Rooms, r)
		}
	}
	for _, r := range d.Rooms {
		round := rng(int32(r.X), int32(r.Y)) < 0.3 // some caverns are rounded
		for y := r.Y; y < r.Y+r.H; y++ {
			for x := r.X; x < r.X+r.W; x++ {
				if round {
					dx := (float64(x-r.X)+.5)/float64(r.W) - .5
					dy := (float64(y-r.Y)+.5)/float64(r.H) - .5
					if dx*dx+dy*dy > .25 {
						continue
					}
				}
				w.set(x, y, DungeonFloor)
			}
		}
	}
	// Corridors: Prim's MST over room centres, then extra loops.
	n := len(d.Rooms)
	in := make([]bool, n)
	in[0] = true
	type edge struct{ a, b int }
	var edges []edge
	for added := 1; added < n; added++ {
		best, ba, bb := math.MaxFloat64, -1, -1
		for a := 0; a < n; a++ {
			if !in[a] {
				continue
			}
			for b := 0; b < n; b++ {
				if in[b] {
					continue
				}
				ax, ay := d.Rooms[a].Center()
				bx, by := d.Rooms[b].Center()
				if dd := math.Hypot(float64(ax-bx), float64(ay-by)); dd < best {
					best, ba, bb = dd, a, b
				}
			}
		}
		in[bb] = true
		edges = append(edges, edge{ba, bb})
	}
	for k := 0; k < 2 && n > 3; k++ {
		a := int(rng(int32(k), 77) * float64(n-1))
		b := int(rng(int32(k), 78) * float64(n-1))
		if a != b {
			edges = append(edges, edge{a, b})
		}
	}
	for k, e := range edges {
		ax, ay := d.Rooms[e.a].Center()
		bx, by := d.Rooms[e.b].Center()
		width := 1 + int(rng(int32(k), 91)*1.99)
		hfirst := rng(int32(k), 92) < 0.5
		carve := func(x, y int) {
			for oy2 := 0; oy2 < width; oy2++ {
				for ox2 := 0; ox2 < width; ox2++ {
					xx, yy := x+ox2, y+oy2
					if xx > ox && yy > oy && xx < ox+S-1 && yy < oy+S-1 && w.at(xx, yy) == Wall {
						w.set(xx, yy, DungeonFloor)
					}
				}
			}
		}
		cx, cy := ax, ay
		if hfirst {
			for ; cx != bx; cx += sign(bx - cx) {
				carve(cx, cy)
			}
			for ; cy != by; cy += sign(by - cy) {
				carve(cx, cy)
			}
		} else {
			for ; cy != by; cy += sign(by - cy) {
				carve(cx, cy)
			}
			for ; cx != bx; cx += sign(bx - cx) {
				carve(cx, cy)
			}
		}
		carve(bx, by)
	}
	// Pillars in big halls (kept away from the centre lines so paths stay open).
	for _, r := range d.Rooms {
		if r.W < 9 || r.H < 9 {
			continue
		}
		for _, p := range [][2]int{{r.X + 2, r.Y + 2}, {r.X + r.W - 3, r.Y + 2}, {r.X + 2, r.Y + r.H - 3}, {r.X + r.W - 3, r.Y + r.H - 3}} {
			if w.at(p[0], p[1]) == DungeonFloor {
				w.set(p[0], p[1], Wall)
			}
		}
	}
	// Entry is room 0; the boss lives in the room farthest by walking distance.
	d.Entry = 0
	ex, ey := d.Rooms[0].Center()
	dist := w.floodDistance(ex, ey, d)
	d.Boss = 0
	far := -1
	for k, r := range d.Rooms {
		x, y := r.Center()
		if v, ok := dist[w.Index(x, y)]; ok && v > far {
			far, d.Boss = v, k
		}
	}
	w.set(ex, ey, Portal)
	// Caches and spawns.
	d.Spawns = d.Spawns[:0]
	order := make([]int, n)
	for k := range order {
		order[k] = k
	}
	sort.Slice(order, func(a, b int) bool { return order[a] < order[b] })
	for _, k := range order {
		if k == d.Entry {
			continue
		}
		r := d.Rooms[k]
		cx, cy := r.Center()
		if k == d.Boss {
			d.Spawns = append(d.Spawns, [3]int{cx, cy, 1})
			if w.at(cx, cy-2) == DungeonFloor {
				w.set(cx, cy-2, Chest)
			}
			if w.at(cx+2, cy-2) == DungeonFloor {
				w.set(cx+2, cy-2, Chest)
			}
			continue
		}
		if rng(int32(k), 31) < 0.6 {
			x, y := r.X+1+int(rng(int32(k), 32)*float64(r.W-2)), r.Y+1+int(rng(int32(k), 33)*float64(r.H-2))
			if w.at(x, y) == DungeonFloor {
				w.set(x, y, Chest)
			}
		}
		guards := 2 + int(rng(int32(k), 34)*2.99)
		for g := 0; g < guards; g++ {
			x, y := r.X+1+int(rng(int32(k*7+g), 35)*float64(r.W-2)), r.Y+1+int(rng(int32(k*7+g), 36)*float64(r.H-2))
			if w.at(x, y) == DungeonFloor {
				d.Spawns = append(d.Spawns, [3]int{x, y, 0})
			}
		}
	}
	// Point the portal pair at the new entry.
	in2 := &w.Portals[i*2]
	out := &w.Portals[i*2+1]
	in2.ToX, in2.ToY = float64(ex)+.5, float64(ey)+.5
	out.X, out.Y = float64(ex)+.5, float64(ey)+.5
}

func sign(v int) int {
	if v < 0 {
		return -1
	}
	if v > 0 {
		return 1
	}
	return 0
}

func (w *World) floodDistance(sx, sy int, d *Dungeon) map[int]int {
	dist := map[int]int{w.Index(sx, sy): 0}
	queue := [][2]int{{sx, sy}}
	for len(queue) > 0 {
		p := queue[0]
		queue = queue[1:]
		for _, o := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
			x, y := p[0]+o[0], p[1]+o[1]
			if !d.Contains(float64(x), float64(y)) || w.at(x, y) == Wall {
				continue
			}
			j := w.Index(x, y)
			if _, seen := dist[j]; !seen {
				dist[j] = dist[w.Index(p[0], p[1])] + 1
				queue = append(queue, [2]int{x, y})
			}
		}
	}
	return dist
}

// DungeonAt returns the dungeon containing a position.
func (w *World) DungeonAt(x, y float64) (*Dungeon, bool) {
	for _, d := range w.Dungeons {
		if d.Contains(x, y) {
			return d, true
		}
	}
	return nil, false
}

func (w *World) Underground(x, y float64) bool {
	_, ok := w.DungeonAt(x, y)
	return ok
}
