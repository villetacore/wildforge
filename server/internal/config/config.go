// Package config collects server settings from flags and environment variables.
package config

import (
	"flag"
	"os"
	"strconv"
)

type Config struct {
	Addr       string // HTTP/WebSocket listen address
	DataPath   string // account database file
	ContentDir string // directory with content JSON files
	Seed       uint32 // world generation seed
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// Load parses command line flags; environment variables provide defaults.
func Load() Config {
	var c Config
	seed, _ := strconv.ParseUint(envOr("WILDFORGE_SEED", "1337"), 10, 32)
	flag.StringVar(&c.Addr, "addr", envOr("WILDFORGE_ADDR", ":7777"), "listen address")
	flag.StringVar(&c.DataPath, "data", envOr("WILDFORGE_DATA", "data/users.json"), "account database file")
	flag.StringVar(&c.ContentDir, "content", envOr("WILDFORGE_CONTENT", "../content"), "content directory")
	s := flag.Uint("seed", uint(seed), "world generation seed")
	flag.Parse()
	c.Seed = uint32(*s)
	return c
}
