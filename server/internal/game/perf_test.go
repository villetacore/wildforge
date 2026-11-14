package game

import (
	"testing"
	"time"
)

func TestTickBudget(t *testing.T) {
	g, p := newTestGame(t)
	n := 0
	for len(p.out) > 0 {
		b := <-p.out
		n += len(b)
	}
	t.Logf("welcome traffic: %.1f MB, enemies %d, npcs %d", float64(n)/1e6, len(g.enemies), len(g.npcs))
	start := time.Now()
	for i := 0; i < 200; i++ {
		g.tick()
		for len(p.out) > 0 {
			<-p.out
		}
	}
	per := time.Since(start) / 200
	t.Logf("tick: %v", per)
	if per > 25*time.Millisecond {
		t.Fatalf("tick too slow: %v", per)
	}
}
