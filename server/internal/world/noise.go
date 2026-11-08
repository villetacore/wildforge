package world

import "math"

// Hash returns a deterministic pseudo-random value in [0, 1] for a cell.
func Hash(x, y int32, seed uint32) float64 {
	h := uint32(x)*374761393 ^ uint32(y)*668265263 ^ seed*2147483647
	h = (h ^ (h >> 13)) * 1274126177
	h ^= h >> 16
	return float64(h&0xFFFF) / 65535
}

func smooth(t float64) float64 { return t * t * (3 - 2*t) }

func valueNoise(x, y float64, seed uint32) float64 {
	xi, yi := int32(math.Floor(x)), int32(math.Floor(y))
	xf, yf := smooth(x-float64(xi)), smooth(y-float64(yi))
	a, b := Hash(xi, yi, seed), Hash(xi+1, yi, seed)
	c, d := Hash(xi, yi+1, seed), Hash(xi+1, yi+1, seed)
	top := a + (b-a)*xf
	bot := c + (d-c)*xf
	return top + (bot-top)*yf
}

// FBM is fractal value noise in roughly [0, 1].
func FBM(x, y float64, seed uint32) float64 {
	sum, amp, freq, norm := 0.0, 1.0, 1.0, 0.0
	for o := uint32(0); o < 5; o++ {
		sum += valueNoise(x*freq, y*freq, seed+o*101) * amp
		norm += amp
		amp *= 0.5
		freq *= 2
	}
	return sum / norm
}
