package world

import (
	"fmt"
	"os"
	"strings"
	"testing"
)

// WILDFORGE_DUMP=x,y,r go test ./internal/world -run Dump -v prints an ASCII map.
func TestDump(t *testing.T) {
	spec := os.Getenv("WILDFORGE_DUMP")
	if spec == "" {
		t.Skip()
	}
	var cx, cy, r int
	fmt.Sscanf(spec, "%d,%d,%d", &cx, &cy, &r)
	w := Generate(1337, loadPack(t))
	chars := ".~s\"=^#_PC*%&,BLbW"
	spots := map[int]byte{}
	for _, s := range w.NPCSpots {
		spots[w.Index(int(s.X), int(s.Y))] = s.Role[0] - 32
	}
	var b strings.Builder
	for y := cy - r; y <= cy+r; y++ {
		for x := cx - r; x <= cx+r; x++ {
			if !w.InBounds(x, y) {
				b.WriteByte(' ')
				continue
			}
			i := w.Index(x, y)
			if c, ok := spots[i]; ok {
				b.WriteByte(c)
			} else if w.Nodes[i] != 0 {
				b.WriteByte('t')
			} else {
				b.WriteByte(chars[w.Terrain[i]])
			}
		}
		b.WriteByte('\n')
	}
	t.Log("\n" + b.String())
}
