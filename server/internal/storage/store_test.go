package storage

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestFailedWritesCanBeRetried(t *testing.T) {
	path := filepath.Join(t.TempDir(), "users.json")
	s, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	// A directory at the database path forces the final rename to fail.
	if err := os.Mkdir(path, 0700); err != nil {
		t.Fatal(err)
	}
	if err := s.Register("alice", "password"); err == nil {
		t.Fatal("registration unexpectedly succeeded")
	}
	if _, err := s.Authenticate("alice", "password"); !errors.Is(err, ErrBadLogin) {
		t.Fatalf("login after failed registration: %v", err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := s.Register("alice", "password"); err != nil {
		t.Fatal(err)
	}
	first := json.RawMessage(`{"level":2}`)
	if err := s.SaveCharacter("alice", first); err != nil {
		t.Fatal(err)
	}
	first[9] = '9'
	backup := path + ".bak"
	if err := os.Rename(path, backup); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(path, 0700); err != nil {
		t.Fatal(err)
	}
	next := json.RawMessage(`{"level":3}`)
	if err := s.SaveCharacter("alice", next); err == nil {
		t.Fatal("save unexpectedly succeeded")
	}
	ch, err := s.Authenticate("alice", "password")
	if err != nil || string(ch) != `{"level":2}` {
		t.Fatalf("after failed save: %s, %v", ch, err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(backup, path); err != nil {
		t.Fatal(err)
	}
	if err := s.SaveCharacter("alice", next); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	ch, err = reopened.Authenticate("alice", "password")
	var saved struct{ Level int }
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(ch, &saved); err != nil || saved.Level != 3 {
		t.Fatalf("after reopening: %s, %v", ch, err)
	}
}
