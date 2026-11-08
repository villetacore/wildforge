// Package netserver is the WebSocket transport. Each connection goes
// through an authentication phase (register/login) and then forwards game
// input to the simulation and game output back to the socket.
package netserver

import (
	"encoding/json"
	"errors"
	"log"
	"net/http"
	"regexp"
	"sync/atomic"
	"time"

	"github.com/gorilla/websocket"

	"github.com/villetacore/wildforge/server/internal/game"
	"github.com/villetacore/wildforge/server/internal/protocol"
	"github.com/villetacore/wildforge/server/internal/storage"
)

var usernameRe = regexp.MustCompile(`^[A-Za-z0-9_]{3,16}$`)

type Server struct {
	game     *game.Game
	store    *storage.Store
	nextID   atomic.Int64
	upgrader websocket.Upgrader
}

func New(g *game.Game, s *storage.Store) *Server {
	return &Server{
		game:  g,
		store: s,
		upgrader: websocket.Upgrader{
			ReadBufferSize:  4096,
			WriteBufferSize: 64 * 1024,
			// Clients are native apps, not browser pages, so any origin is fine.
			CheckOrigin: func(*http.Request) bool { return true },
		},
	}
}

func validate(m protocol.Auth) error {
	if !usernameRe.MatchString(m.Username) {
		return errors.New("username must be 3-16 letters, digits or _")
	}
	if len(m.Password) < 4 || len(m.Password) > 128 {
		return errors.New("password must be 4-128 characters")
	}
	return nil
}

func send(ws *websocket.Conn, v any) {
	ws.SetWriteDeadline(time.Now().Add(5 * time.Second))
	_ = ws.WriteJSON(v)
}

// authenticate loops until the client logs in successfully. Registering does
// not log in, so clients send "login" after "registered".
func (s *Server) authenticate(ws *websocket.Conn) (string, json.RawMessage, bool) {
	for {
		ws.SetReadDeadline(time.Now().Add(2 * time.Minute))
		var m protocol.Auth
		if err := ws.ReadJSON(&m); err != nil {
			return "", nil, false
		}
		if err := validate(m); err != nil {
			send(ws, protocol.NewError(err.Error()))
			continue
		}
		switch m.T {
		case "register":
			if err := s.store.Register(m.Username, m.Password); err != nil {
				send(ws, protocol.NewError(err.Error()))
				continue
			}
			log.Printf("registered %s", m.Username)
			send(ws, protocol.Registered{T: "registered", Username: m.Username})
		case "login":
			ch, err := s.store.Authenticate(m.Username, m.Password)
			if err != nil {
				send(ws, protocol.NewError(err.Error()))
				continue
			}
			return m.Username, ch, true
		default:
			send(ws, protocol.NewError("log in first"))
		}
	}
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	ws, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer ws.Close()
	ws.SetReadLimit(8 * 1024)

	name, ch, ok := s.authenticate(ws)
	if !ok {
		return
	}

	id := s.nextID.Add(1)
	out := make(chan []byte, 256)
	s.game.Join(id, name, ch, out)

	writerDone := make(chan struct{})
	go func() {
		defer close(writerDone)
		for b := range out {
			ws.SetWriteDeadline(time.Now().Add(5 * time.Second))
			if err := ws.WriteMessage(websocket.TextMessage, b); err != nil {
				break
			}
		}
		// The game closed our channel (left or rejected) or the socket broke.
		ws.WriteControl(websocket.CloseMessage, websocket.FormatCloseMessage(websocket.CloseNormalClosure, ""), time.Now().Add(time.Second))
		ws.Close()
		for range out { // drain so the game never blocks on us
		}
	}()

	for {
		ws.SetReadDeadline(time.Now().Add(5 * time.Minute))
		_, data, err := ws.ReadMessage()
		if err != nil {
			break
		}
		var m protocol.ClientMsg
		if json.Unmarshal(data, &m) == nil {
			s.game.Input(id, m)
		}
	}
	s.game.Leave(id)
	<-writerDone
}
