// Command wildforge-server runs the Wildforge game server: accounts,
// persistence and the realtime world, all behind one WebSocket endpoint.
package main

import (
	"context"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"os/signal"
	"strconv"
	"syscall"
	"time"

	"github.com/villetacore/wildforge/server/internal/config"
	"github.com/villetacore/wildforge/server/internal/content"
	"github.com/villetacore/wildforge/server/internal/game"
	"github.com/villetacore/wildforge/server/internal/netserver"
	"github.com/villetacore/wildforge/server/internal/storage"
	"github.com/villetacore/wildforge/server/internal/world"
)

func main() {
	cfg := config.Load()

	pack, err := content.Load(cfg.ContentDir)
	if err != nil {
		log.Fatalf("load content from %s: %v", cfg.ContentDir, err)
	}
	store, err := storage.Open(cfg.DataPath)
	if err != nil {
		log.Fatalf("open storage: %v", err)
	}
	w := world.Generate(cfg.Seed, pack)
	g := game.New(pack, w, store)

	stop := make(chan struct{})
	done := make(chan struct{})
	go func() { g.Run(stop); close(done) }()

	mux := http.NewServeMux()
	mux.Handle("GET /ws", netserver.New(g, store))
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, _ *http.Request) { io.WriteString(w, "ok") })
	mux.HandleFunc("GET /api/leaderboard", func(w http.ResponseWriter, r *http.Request) {
		limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
		if limit <= 0 || limit > 100 {
			limit = 10
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(store.Leaderboard(limit))
	})

	srv := &http.Server{Addr: cfg.Addr, Handler: mux}
	go func() {
		log.Printf("wildforge server on %s (ws path /ws) | content %s: %d items, %d resources, %d recipes, %d monster families, %d gear bases, %d spells, %d zones | world %dx%d seed %d, %d structures, %d NPC spots",
			cfg.Addr, cfg.ContentDir, len(pack.Items), len(pack.Resources), len(pack.Recipes), len(pack.Bestiary.Families), len(pack.Gear.Bases),
			len(pack.Spells), len(pack.Zones), w.Size, w.Size, cfg.Seed, len(w.Structures), len(w.NPCSpots))
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatal(err)
		}
	}()

	ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer cancel()
	<-ctx.Done()
	log.Println("shutting down...")
	close(stop)
	<-done
	shutdownCtx, c2 := context.WithTimeout(context.Background(), 2*time.Second)
	defer c2()
	srv.Shutdown(shutdownCtx)
}
