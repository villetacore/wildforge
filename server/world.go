package main

import (
	"math"
	"strings"
)

// Procedural tile map. The server generates it once and sends it to clients.

const (
	MapW = 256
	MapH = 256
)

type Tile byte

const (
	Grass Tile = iota
	Water
	Sand
	Tree
	Stone
	Forest
	Road
)

func (t Tile) Walkable() bool { return t != Water && t != Tree && t != Stone }

type World struct {
	Tiles          []Tile
	SpawnX, SpawnY float64
}

func hash2(x, y int32, seed uint32) float64 {
	h := uint32(x)*374761393 ^ uint32(y)*668265263 ^ seed*2147483647
	h = (h ^ (h >> 13)) * 1274126177
	h ^= h >> 16
	return float64(h&0xFFFF) / 65535
}

func smooth(t float64) float64 { return t * t * (3 - 2*t) }

func valueNoise(x, y float64, seed uint32) float64 {
	xi, yi := int32(math.Floor(x)), int32(math.Floor(y))
	xf, yf := smooth(x-float64(xi)), smooth(y-float64(yi))
	a, b := hash2(xi, yi, seed), hash2(xi+1, yi, seed)
	c, d := hash2(xi, yi+1, seed), hash2(xi+1, yi+1, seed)
	top := a + (b-a)*xf
	bot := c + (d-c)*xf
	return top + (bot-top)*yf
}

func fbm(x, y float64, seed uint32) float64 {
	sum, amp, freq, norm := 0.0, 1.0, 1.0, 0.0
	for o := uint32(0); o < 5; o++ {
		sum += valueNoise(x*freq, y*freq, seed+o*101) * amp
		norm += amp
		amp *= 0.5
		freq *= 2
	}
	return sum / norm
}

func GenerateWorld(seed uint32) *World {
	w := &World{Tiles: make([]Tile, MapW*MapH)}
	cx, cy := MapW/2.0, MapH/2.0
	for y := 0; y < MapH; y++ {
		for x := 0; x < MapW; x++ {
			fx, fy := float64(x), float64(y)
			e := fbm(fx/48, fy/48, seed)
			// Sink the borders so the world is an island.
			dx, dy := (fx-cx)/cx, (fy-cy)/cy
			d := math.Sqrt(dx*dx + dy*dy)
			e = e + 0.22 - math.Pow(d, 2.8)*0.6
			m := fbm(fx/32+500, fy/32+500, seed^0xBEEF)
			rock := fbm(fx/16+900, fy/16+900, seed^0xC0DE)
			r := hash2(int32(x), int32(y), seed^0x1234)
			town := math.Hypot(fx-cx, fy-cy) < 10
			t := Grass
			switch {
			case e < 0.32:
				t = Water
			case e < 0.36:
				t = Sand
			case town:
				t = Grass
			case rock > 0.62 && e > 0.5:
				if r < 0.75 {
					t = Stone
				}
			case m > 0.56:
				t = Forest
				if r < 0.45 {
					t = Tree
				}
			case r < 0.03:
				t = Tree
			}
			w.Tiles[y*MapW+x] = t
		}
	}
	// Roads through the centre keep the start area open and connected.
	for i := 0; i < MapW; i++ {
		for off := 0; off < 2; off++ {
			for _, p := range [][2]int{{i, MapH/2 + off}, {MapW/2 + off, i}} {
				if idx := p[1]*MapW + p[0]; w.Tiles[idx] != Water {
					w.Tiles[idx] = Road
				}
			}
		}
	}
	for y := MapH/2 - 3; y < MapH/2+4; y++ {
		for x := MapW/2 - 3; x < MapW/2+4; x++ {
			w.Tiles[y*MapW+x] = Road
		}
	}
	w.SpawnX, w.SpawnY = cx+0.5, cy+0.5
	return w
}

func (w *World) Tile(x, y int) Tile {
	if x < 0 || y < 0 || x >= MapW || y >= MapH {
		return Water
	}
	return w.Tiles[y*MapW+x]
}

// Free reports whether a circle of radius 0.3 at (x, y) fits on walkable tiles.
func (w *World) Free(x, y float64) bool {
	const r = 0.3
	for _, o := range [][2]float64{{-r, -r}, {r, -r}, {-r, r}, {r, r}} {
		if !w.Tile(int(math.Floor(x+o[0])), int(math.Floor(y+o[1]))).Walkable() {
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

func (w *World) Encode() string {
	var b strings.Builder
	b.Grow(len(w.Tiles))
	for _, t := range w.Tiles {
		b.WriteByte('0' + byte(t))
	}
	return b.String()
}
