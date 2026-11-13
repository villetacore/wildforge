// Package storage keeps player accounts and their saved characters in a JSON
// file. Characters are opaque JSON so the game package owns their schema.
package storage

import (
	"crypto/pbkdf2"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"sync"
	"time"
)

var (
	ErrUserExists = errors.New("this username is already taken")
	ErrBadLogin   = errors.New("invalid username or password")
)

type User struct {
	Name      string          `json:"name"`
	Salt      string          `json:"salt"`
	Hash      string          `json:"hash"`
	CreatedAt time.Time       `json:"created_at"`
	Character json.RawMessage `json:"character,omitempty"`
}

type Store struct {
	mu    sync.Mutex
	path  string
	users map[string]*User
}

func Open(path string) (*Store, error) {
	s := &Store{path: path, users: map[string]*User{}}
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return s, nil
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal(data, &s.users); err != nil {
		return nil, err
	}
	if s.users == nil {
		return nil, errors.New("expected an account table")
	}
	for name, user := range s.users {
		if user == nil {
			return nil, errors.New("empty account record for " + name)
		}
	}
	return s, nil
}

func hashPassword(password string, salt []byte) []byte {
	key, err := pbkdf2.Key(sha256.New, password, salt, 100_000, 32)
	if err != nil {
		panic(err)
	}
	return key
}

// saveLocked writes the user table atomically. Caller holds s.mu.
func (s *Store) saveLocked() error {
	data, err := json.MarshalIndent(s.users, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(s.path), ".wildforge-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(tmp.Name())
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmp.Name(), s.path)
}

func (s *Store) Register(name, password string) error {
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return err
	}
	hash := hashPassword(password, salt) // slow on purpose: do it outside the lock
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.users[name]; ok {
		return ErrUserExists
	}
	s.users[name] = &User{Name: name, Salt: hex.EncodeToString(salt), Hash: hex.EncodeToString(hash), CreatedAt: time.Now().UTC()}
	if err := s.saveLocked(); err != nil {
		delete(s.users, name)
		return err
	}
	return nil
}

// Authenticate checks the password and returns the saved character JSON
// (nil for a new account).
func (s *Store) Authenticate(name, password string) (json.RawMessage, error) {
	s.mu.Lock()
	u, ok := s.users[name]
	var salt, want []byte
	var ch json.RawMessage
	if ok {
		salt, _ = hex.DecodeString(u.Salt)
		want, _ = hex.DecodeString(u.Hash)
		ch = slices.Clone(u.Character)
	}
	s.mu.Unlock()
	if !ok {
		hashPassword(password, []byte("0123456789abcdef")) // same timing as a real check
		return nil, ErrBadLogin
	}
	if subtle.ConstantTimeCompare(hashPassword(password, salt), want) != 1 {
		return nil, ErrBadLogin
	}
	return ch, nil
}

func (s *Store) SaveCharacter(name string, character json.RawMessage) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	u, ok := s.users[name]
	if !ok {
		return errors.New("unknown user " + name)
	}
	previous := u.Character
	u.Character = slices.Clone(character)
	if err := s.saveLocked(); err != nil {
		u.Character = previous
		return err
	}
	return nil
}

type LeaderboardEntry struct {
	Name  string `json:"name"`
	Level int    `json:"level"`
	XP    int    `json:"xp"`
	Kills int    `json:"kills"`
}

func (s *Store) Leaderboard(limit int) []LeaderboardEntry {
	s.mu.Lock()
	out := make([]LeaderboardEntry, 0, len(s.users))
	for _, u := range s.users {
		e := LeaderboardEntry{Name: u.Name, Level: 1}
		_ = json.Unmarshal(u.Character, &e)
		e.Name = u.Name
		out = append(out, e)
	}
	s.mu.Unlock()
	slices.SortFunc(out, func(a, b LeaderboardEntry) int {
		if a.Level != b.Level {
			return b.Level - a.Level
		}
		return b.XP - a.XP
	})
	return out[:min(limit, len(out))]
}
