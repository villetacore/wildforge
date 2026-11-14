// Command texgen procedurally generates the seamless (tileable) PNG textures
// in content/textures. Run it again after changing a recipe here:
//
//	go run ./cmd/texgen -out ../content/textures
package main

import (
	"flag"
	"image"
	"image/color"
	"image/png"
	"log"
	"math"
	"os"
	"path/filepath"
)

// N is the texture size in pixels. Shaders are written for a 128 px design
// grid; px/py are scaled so higher resolutions add detail, not repeats.
var N = 512

type rgb struct{ r, g, b float64 }

func c(hex uint32) rgb {
	return rgb{float64(hex>>16&0xFF) / 255, float64(hex>>8&0xFF) / 255, float64(hex&0xFF) / 255}
}

func mix(a, b rgb, t float64) rgb {
	t = clamp(t)
	return rgb{a.r + (b.r-a.r)*t, a.g + (b.g-a.g)*t, a.b + (b.b-a.b)*t}
}

func (a rgb) mul(k float64) rgb { return rgb{a.r * k, a.g * k, a.b * k} }

func clamp(v float64) float64 { return math.Max(0, math.Min(1, v)) }

// ---- tileable noise: lattice coordinates wrap at the period

func hash(x, y int, seed uint32) float64 {
	h := uint32(x)*374761393 ^ uint32(y)*668265263 ^ seed*2147483647
	h = (h ^ (h >> 13)) * 1274126177
	h ^= h >> 16
	return float64(h&0xFFFF) / 65535
}

func wrap(v, p int) int { return ((v % p) + p) % p }

// S converts design-grid units (128 px textures) to pixels at the current N.
func S() float64 { return float64(N) / 128 }

func noise(x, y float64, period int, seed uint32) float64 {
	xi, yi := int(math.Floor(x)), int(math.Floor(y))
	xf, yf := x-float64(xi), y-float64(yi)
	xf, yf = xf*xf*(3-2*xf), yf*yf*(3-2*yf)
	h := func(i, j int) float64 { return hash(wrap(i, period), wrap(j, period), seed) }
	a, b, cc, d := h(xi, yi), h(xi+1, yi), h(xi, yi+1), h(xi+1, yi+1)
	return (a+(b-a)*xf)*(1-yf) + (cc+(d-cc)*xf)*yf
}

// fbm samples noise at pixel (px, py) starting with `cells` lattice cells per texture.
func fbm(px, py float64, cells, octaves int, seed uint32) float64 {
	sum, amp, norm := 0.0, 1.0, 0.0
	for o := 0; o < octaves; o++ {
		k := cells << o
		sum += noise(px*float64(k)/float64(N), py*float64(k)/float64(N), k, seed+uint32(o)*31) * amp
		norm += amp
		amp *= 0.5
	}
	return sum / norm
}

// voronoi returns distances to the nearest and second nearest feature point,
// with `cells` cells per texture side (tileable).
func voronoi(px, py float64, cells int, seed uint32) (float64, float64) {
	x, y := px*float64(cells)/float64(N), py*float64(cells)/float64(N)
	xi, yi := int(math.Floor(x)), int(math.Floor(y))
	d1, d2 := 9.0, 9.0
	for j := -1; j <= 1; j++ {
		for i := -1; i <= 1; i++ {
			cx, cy := xi+i, yi+j
			fx := float64(cx) + hash(wrap(cx, cells), wrap(cy, cells), seed)
			fy := float64(cy) + hash(wrap(cx, cells), wrap(cy, cells), seed+7)
			d := math.Hypot(fx-x, fy-y)
			if d < d1 {
				d1, d2 = d, d1
			} else if d < d2 {
				d2 = d
			}
		}
	}
	return d1, d2
}

type shader func(x, y float64) rgb

func stone(x, y float64) rgb {
	d1, d2 := voronoi(x, y, 5, 16)
	n := fbm(x, y, 8, 4, 17)
	col := mix(c(0x6c6c70), c(0xa4a4a8), n*0.7+d1*0.4)
	if d2-d1 < 0.05 {
		col = col.mul(0.65)
	}
	return col
}

var textures = map[string]shader{
	"grass": func(x, y float64) rgb {
		n := fbm(x, y, 4, 4, 1)
		blade := hash(int(x), int(y)/3, 2)
		col := mix(c(0x3f7a2c), c(0x6aa83f), n)
		if blade > 0.86 {
			col = col.mul(1.18)
		} else if blade < 0.1 {
			col = col.mul(0.82)
		}
		return col
	},
	"forest": func(x, y float64) rgb {
		n := fbm(x, y, 6, 4, 3)
		col := mix(c(0x2c4a1f), c(0x4d6b2a), n)
		if v := hash(int(x)/2, int(y)/2, 4); v > 0.93 {
			col = mix(col, c(0x8a6a30), 0.7) // fallen leaves
		}
		return col
	},
	"sand": func(x, y float64) rgb {
		n := fbm(x, y, 8, 3, 5)
		grain := hash(int(x), int(y), 6)*0.12 - 0.06
		ripple := math.Sin((y+fbm(x, y, 2, 2, 7)*30)*2*math.Pi/16) * 0.04
		return mix(c(0xcdb67a), c(0xead79e), n+grain+ripple)
	},
	"water": func(x, y float64) rgb {
		n := fbm(x, y, 4, 4, 8)
		wave := math.Sin((x+y+n*40)*2*math.Pi/32)*0.5 + 0.5
		col := mix(c(0x1d4f8c), c(0x2f78b8), n*0.7+wave*0.3)
		if wave > 0.93 && n > 0.5 {
			col = mix(col, c(0xbfe3ff), 0.5)
		}
		return col
	},
	"road": func(x, y float64) rgb {
		n := fbm(x, y, 6, 4, 9)
		col := mix(c(0x7a5c3a), c(0xa58256), n)
		if d1, _ := voronoi(x, y, 16, 10); d1 < 0.18 {
			col = mix(col, c(0x8d8a80), 0.8) // pebbles
		}
		return col
	},
	"rocky": func(x, y float64) rgb {
		n := fbm(x, y, 5, 5, 11)
		col := mix(c(0x5e5a52), c(0x8a857a), n)
		if d1, d2 := voronoi(x, y, 6, 12); d2-d1 < 0.06 {
			col = col.mul(0.7) // cracks
		}
		return col
	},
	"bark": func(x, y float64) rgb {
		n := fbm(x*4, y*0.5, 2, 4, 13)
		lines := math.Abs(math.Sin((x/float64(N)*12 + n*2) * math.Pi))
		col := mix(c(0x3d2814), c(0x7a5432), n)
		return col.mul(0.75 + 0.25*lines)
	},
	"leaves": func(x, y float64) rgb {
		d1, d2 := voronoi(x, y, 10, 14)
		n := fbm(x, y, 4, 3, 15)
		col := mix(c(0x1f4d1c), c(0x4c8a32), (1-d1)*0.8+n*0.4)
		if d2-d1 < 0.05 {
			col = col.mul(0.6)
		}
		return col
	},
	"stone": stone,
	"iron_ore": func(x, y float64) rgb {
		col := stone(x, y)
		if d1, _ := voronoi(x, y, 12, 18); d1 < 0.22 {
			col = mix(c(0xb35a24), c(0xe09050), fbm(x, y, 8, 2, 19))
		}
		return col
	},
	"crystal": func(x, y float64) rgb {
		d1, d2 := voronoi(x, y, 4, 20)
		col := mix(c(0x3a7bd5), c(0xa8f0ff), d1*1.2)
		col = mix(col, c(0xd9a8ff), fbm(x, y, 3, 2, 21)*0.5)
		if d2-d1 < 0.04 {
			col = mix(col, c(0xffffff), 0.6)
		}
		return col
	},
	"slime": func(x, y float64) rgb {
		d1, _ := voronoi(x, y, 8, 22)
		col := mix(c(0x3fa83a), c(0x8ef06a), fbm(x, y, 4, 3, 23))
		if d1 < 0.15 {
			col = mix(col, c(0xd8ffc0), 0.5) // bubbles
		}
		return col
	},
	"fur": func(x, y float64) rgb {
		n := fbm(x*0.6, y*3, 4, 4, 24)
		strand := hash(int(x), int(y)/4, 25)
		return mix(c(0x5a5a64), c(0xa8a8b4), n*0.8+strand*0.3)
	},
	"bone": func(x, y float64) rgb {
		n := fbm(x, y, 6, 4, 26)
		col := mix(c(0xcfc6a8), c(0xf2ecd8), n)
		if hash(int(x)/2, int(y)/2, 27) > 0.97 {
			col = col.mul(0.8)
		}
		return col
	},
	"skin": func(x, y float64) rgb {
		n := fbm(x, y, 5, 4, 28)
		d1, _ := voronoi(x, y, 14, 29)
		return mix(c(0x6e4466), c(0xa87298), n).mul(0.85 + 0.15*d1)
	},
	"cloth": func(x, y float64) rgb {
		weave := 0.5 + 0.25*math.Sin(x*math.Pi/2) + 0.25*math.Sin(y*math.Pi/2)
		n := fbm(x, y, 8, 3, 30)
		v := 0.75 + 0.15*weave + 0.1*n
		return rgb{v, v, v} // tinted by the material color
	},
	"metal": func(x, y float64) rgb {
		n := fbm(x*0.3, y*4, 4, 4, 31)
		v := 0.6 + 0.3*n
		return rgb{v, v, v * 1.04}
	},
	"wood": func(x, y float64) rgb { // planks for houses / handles
		plank := int(y) / 32
		n := fbm(x*0.4+float64(plank)*40, y*3, 4, 4, uint32(32+plank))
		col := mix(c(0x7a5230), c(0xb4824e), n)
		if int(y)%32 < 2 {
			col = col.mul(0.5)
		}
		return col
	},
}

// detail adds fine high-frequency variation that only shows at high resolution.
func detail(x, y float64, seed uint32) float64 { return fbm(x, y, 64, 3, seed) - 0.5 }

// extra are the textures added with zones and towns. They are generated at
// full resolution; the hand-painted atlas textures are left alone.
var extra = map[string]shader{
	"snow": func(x, y float64) rgb {
		n := fbm(x, y, 4, 5, 40)
		col := mix(c(0xc8d4e4), c(0xf6f9ff), n*0.9+0.2)
		if hash(int(x), int(y), 41) > 0.996 {
			col = c(0xffffff) // sparkle
		}
		return col.mul(1 + detail(x, y, 42)*0.08)
	},
	"swamp": func(x, y float64) rgb {
		n := fbm(x, y, 5, 5, 43)
		col := mix(c(0x2e3a22), c(0x5a6a38), n)
		if d1, _ := voronoi(x, y, 7, 44); d1 < 0.22 && n < 0.5 {
			col = mix(col, c(0x2a3a34), 0.7) // puddles
		}
		return col.mul(1 + detail(x, y, 45)*0.15)
	},
	"ash": func(x, y float64) rgb {
		n := fbm(x, y, 6, 5, 46)
		col := mix(c(0x2a2624), c(0x5a524c), n)
		if d1, d2 := voronoi(x, y, 9, 47); d2-d1 < 0.04 {
			col = mix(col, c(0xc0401a), 0.55) // glowing cracks
		}
		return col.mul(1 + detail(x, y, 48)*0.12)
	},
	"lava": func(x, y float64) rgb {
		n := fbm(x, y, 3, 5, 49)
		d1, d2 := voronoi(x, y, 5, 50)
		col := mix(c(0xff3a00), c(0xffd040), n)
		if d2-d1 > 0.18 {
			col = mix(col, c(0x2a1a14), smoothCrust(d2-d1))
		}
		return col
	},
	"cobble": func(x, y float64) rgb {
		d1, d2 := voronoi(x, y, 14, 51)
		n := fbm(x, y, 10, 3, 52)
		stone := hash(int(math.Floor(x*14/float64(N))), int(math.Floor(y*14/float64(N))), 53)
		col := mix(c(0x5c5a58), c(0x9a948a), 0.35+n*0.5+stone*0.2)
		col = col.mul(1.05 - d1*0.35)
		if d2-d1 < 0.07 {
			col = mix(c(0x2a2622), c(0x4a4038), n) // mortar and dirt
		}
		return col.mul(1 + detail(x, y, 54)*0.1)
	},
	"roof_tiles": func(x, y float64) rgb { // terracotta pantiles, tinted per house
		row := int(y / (8 * S()))
		off := 0.0
		if row%2 == 1 {
			off = 4 * S()
		}
		u := math.Mod(x+off, 8*S()) / (8 * S())
		v := math.Mod(y, 8*S()) / (8 * S())
		curve := math.Sin(u*math.Pi)*0.25 + 0.75
		n := fbm(x, y, 8, 4, 55)
		col := mix(c(0xb8b0a8), c(0xffffff), n*0.4+0.6).mul(curve)
		if v > 0.85 {
			col = col.mul(0.55) // overlap shadow
		}
		return col.mul(1 + detail(x, y, 56)*0.08)
	},
	"slate": func(x, y float64) rgb {
		row := int(y / (6 * S()))
		off := float64(row%2) * 5 * S()
		u := math.Mod(x+off, 10*S()) / (10 * S())
		v := math.Mod(y, 6*S()) / (6 * S())
		n := fbm(x, y, 6, 4, 57)
		col := mix(c(0x9a9aa2), c(0xd8d8e0), n)
		if u < 0.06 || v > 0.88 {
			col = col.mul(0.5)
		}
		return col
	},
	"marble": func(x, y float64) rgb {
		n := fbm(x, y, 3, 6, 58)
		vein := math.Abs(math.Sin((x/float64(N)*6 + n*5) * math.Pi))
		col := mix(c(0xe8e4dc), c(0xfaf8f4), n)
		if vein < 0.06 {
			col = mix(col, c(0x8a8478), 0.6)
		}
		return col
	},
	"sandstone": func(x, y float64) rgb {
		band := fbm(x*0.2, y*3, 2, 4, 59)
		n := fbm(x, y, 8, 4, 60)
		col := mix(c(0xc8a068), c(0xe8c890), band*0.7+n*0.3)
		row := int(y / (16 * S()))
		if math.Mod(y, 16*S()) < 1.2*S() || math.Mod(x+float64(row%2)*16*S(), 32*S()) < 1.2*S() {
			col = col.mul(0.7)
		}
		return col
	},
	"timber": func(x, y float64) rgb { // half-timbered wall: plaster with dark beams, tinted
		bx := math.Mod(x, 64*S()) / (64 * S())
		by := math.Mod(y, 64*S()) / (64 * S())
		n := fbm(x, y, 8, 4, 61)
		beam := bx < 0.08 || by < 0.08 || math.Abs(bx-by) < 0.05
		if beam {
			return mix(c(0x3a2818), c(0x5a3c22), fbm(x*0.3, y*3, 4, 3, 62))
		}
		return mix(c(0xd8d0c0), c(0xf8f4ec), n)
	},
	"moss": func(x, y float64) rgb {
		n := fbm(x, y, 6, 5, 63)
		d1, _ := voronoi(x, y, 20, 64)
		return mix(c(0x2a4a1a), c(0x6a8a3a), n*0.8+(1-d1)*0.3).mul(1 + detail(x, y, 65)*0.2)
	},
	"ice": func(x, y float64) rgb {
		n := fbm(x, y, 3, 5, 66)
		d1, d2 := voronoi(x, y, 6, 67)
		col := mix(c(0x7ab0d8), c(0xd8f0ff), n*0.7+d1*0.3)
		if d2-d1 < 0.03 {
			col = mix(col, c(0xffffff), 0.7)
		}
		return col
	},
	"steel": func(x, y float64) rgb { // clean brushed metal, tinted per item
		brush := fbm(x*0.08, y*6, 4, 4, 69)
		n := fbm(x, y, 8, 3, 70)
		v := 0.72 + 0.16*brush + 0.08*n
		if hash(int(x), int(y), 71) > 0.995 {
			v -= 0.25 // tiny scratches
		}
		return rgb{v, v, v * 1.02}
	},
	"window": func(x, y float64) rgb { // leaded glass, used on houses at night
		u := math.Mod(x, 16*S()) / (16 * S())
		v := math.Mod(y, 16*S()) / (16 * S())
		if u < 0.08 || v < 0.08 {
			return c(0x2a2420)
		}
		return mix(c(0xffc060), c(0xffe8a0), fbm(x, y, 4, 2, 68))
	},
}

func smoothCrust(t float64) float64 { t = clamp((t - 0.18) * 6); return t * t * (3 - 2*t) }

func main() {
	out := flag.String("out", "../content/textures", "output directory")
	legacy := flag.Bool("legacy", false, "also regenerate the original procedural set (overwrites the painted atlas textures)")
	size := flag.Int("size", 512, "texture size in pixels")
	flag.Parse()
	N = *size
	if err := os.MkdirAll(*out, 0o755); err != nil {
		log.Fatal(err)
	}
	set := map[string]shader{}
	for k, v := range extra {
		set[k] = v
	}
	if *legacy {
		for k, v := range textures {
			set[k] = v
		}
	}
	for name, shade := range set {
		img := image.NewNRGBA(image.Rect(0, 0, N, N))
		for y := 0; y < N; y++ {
			for x := 0; x < N; x++ {
				col := shade(float64(x), float64(y))
				img.SetNRGBA(x, y, color.NRGBA{uint8(clamp(col.r) * 255), uint8(clamp(col.g) * 255), uint8(clamp(col.b) * 255), 255})
			}
		}
		f, err := os.Create(filepath.Join(*out, name+".png"))
		if err != nil {
			log.Fatal(err)
		}
		if err := png.Encode(f, img); err != nil {
			log.Fatal(err)
		}
		f.Close()
	}
	log.Printf("wrote %d textures to %s", len(set), *out)
}
