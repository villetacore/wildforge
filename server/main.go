// Command server is the Wildforge game server: accounts, persistence and the
// realtime world simulation, all over a single WebSocket endpoint.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"io"
	"log"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"syscall"
	"time"
)

func main() {
	addr := flag.String("addr", envOr("WILDFORGE_ADDR", ":7777"), "listen address")
	dataPath := flag.String("data", envOr("WILDFORGE_DATA", "data/users.json"), "path to the account database file")
	seed := flag.Uint("seed", 1337, "world generation seed")
	flag.Parse()

	store, err := OpenStore(*dataPath)
	if err != nil {
		log.Fatalf("open store: %v", err)
	}
	game := NewGame(uint32(*seed), store)
	stop, done := make(chan struct{}), make(chan struct{})
	go game.Run(stop, done)

	mux := http.NewServeMux()
	mux.HandleFunc("GET /ws", handleWS(game, store))
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, _ *http.Request) { io.WriteString(w, "ok") })
	mux.HandleFunc("GET /api/leaderboard", func(w http.ResponseWriter, r *http.Request) {
		limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
		if limit <= 0 || limit > 100 {
			limit = 10
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(store.Leaderboard(limit))
	})

	srv := &http.Server{Addr: *addr, Handler: mux}
	go func() {
		log.Printf("wildforge server listening on %s (ws://<host>%s/ws, data: %s, seed: %d)", *addr, *addr, *dataPath, *seed)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatal(err)
		}
	}()

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	<-ctx.Done()
	log.Println("shutting down...")
	close(stop)
	<-done
	shutdownCtx, c2 := context.WithTimeout(context.Background(), 2*time.Second)
	defer c2()
	srv.Shutdown(shutdownCtx)
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
