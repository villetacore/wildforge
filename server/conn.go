package main

import (
	"encoding/json"
	"errors"
	"log"
	"net/http"
	"regexp"
	"sync/atomic"
	"time"

	"github.com/gorilla/websocket"
)

var (
	usernameRe   = regexp.MustCompile(`^[A-Za-z0-9_]{3,16}$`)
	nextPlayerID atomic.Int64
	upgrader     = websocket.Upgrader{
		ReadBufferSize:  4096,
		WriteBufferSize: 64 * 1024,
		// The game client is a native app, not a browser page, so any origin is fine.
		CheckOrigin: func(*http.Request) bool { return true },
	}
)

type authMsg struct {
	T        string `json:"t"`
	Username string `json:"username"`
	Password string `json:"password"`
}

func validateCredentials(m authMsg) error {
	if !usernameRe.MatchString(m.Username) {
		return errors.New("username must be 3-16 letters, digits or _")
	}
	if len(m.Password) < 4 || len(m.Password) > 128 {
		return errors.New("password must be 4-128 characters")
	}
	return nil
}

func sendJSON(ws *websocket.Conn, v any) error {
	ws.SetWriteDeadline(time.Now().Add(5 * time.Second))
	return ws.WriteJSON(v)
}

func errMsg(text string) map[string]string { return map[string]string{"t": "error", "message": text} }

// handleWS runs one client connection: register/login first, then game input.
func handleWS(g *Game, store *Store) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ws, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer ws.Close()
		ws.SetReadLimit(8 * 1024)

		// Phase 1: authentication. The client may register any number of
		// times (e.g. after a typo) and then must log in.
		var name string
		var ch *Character
		for name == "" {
			ws.SetReadDeadline(time.Now().Add(2 * time.Minute))
			var m authMsg
			if err := ws.ReadJSON(&m); err != nil {
				return
			}
			if err := validateCredentials(m); err != nil {
				sendJSON(ws, errMsg(err.Error()))
				continue
			}
			switch m.T {
			case "register":
				if err := store.Register(m.Username, m.Password); err != nil {
					sendJSON(ws, errMsg(err.Error()))
					continue
				}
				log.Printf("registered %s", m.Username)
				sendJSON(ws, map[string]string{"t": "registered", "username": m.Username})
			case "login":
				c, err := store.Authenticate(m.Username, m.Password)
				if err != nil {
					sendJSON(ws, errMsg(err.Error()))
					continue
				}
				name, ch = m.Username, c
			default:
				sendJSON(ws, errMsg("log in first"))
			}
		}

		// Phase 2: in game.
		id := nextPlayerID.Add(1)
		out := make(chan []byte, 256)
		g.events <- Event{Kind: "join", ID: id, Name: name, Character: ch, Out: out}

		writerDone := make(chan struct{})
		go func() {
			defer close(writerDone)
			for b := range out {
				ws.SetWriteDeadline(time.Now().Add(5 * time.Second))
				if err := ws.WriteMessage(websocket.TextMessage, b); err != nil {
					break
				}
			}
			// Game closed our channel (left or rejected) or the socket broke.
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
			var m ClientMsg
			if json.Unmarshal(data, &m) == nil {
				g.events <- Event{Kind: "input", ID: id, Msg: m}
			}
		}
		g.events <- Event{Kind: "leave", ID: id}
		<-writerDone
	}
}
