#!/usr/bin/env sh
# Build and start the Wildforge game server (accounts + world) on :7777.
set -e
cd "$(dirname "$0")/../server"
go build -o wildforge-server ./cmd/wildforge-server
exec ./wildforge-server -content ../content "$@"
