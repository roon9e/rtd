// Command vectors emits shared MTProto vectors from the reference Go
// implementation (`td`) so the Rust implementation can be cross-checked
// against it. Run from this directory with:
//
//	go run . > ../vectors.json
//
// The output is a single dependency-free JSON document, so the Rust consumer
// needs no extra crates to read it.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/iamxvbaba/td/bin"
	"github.com/iamxvbaba/td/crypto"
	"github.com/iamxvbaba/td/proto"
)

type vector struct {
	Name string `json:"name"`
	A    string `json:"a,omitempty"`
	B    string `json:"b,omitempty"`
	C    string `json:"c,omitempty"`
	I    int64  `json:"i,omitempty"`
	I2   int64  `json:"i2,omitempty"`
	I3   int64  `json:"i3,omitempty"`
}

func hexs(b []byte) string { return hex.EncodeToString(b) }

func main() {
	var out []vector

	authKey := crypto.Key{}
	for i := range authKey {
		authKey[i] = byte(i)
	}

	// A plaintext-with-padding blob: 76 deterministic bytes plus 16 zero bytes.
	padded := make([]byte, 0, 92)
	padded = append(padded, []byte(strings.Repeat("a", 76))...)
	padded = append(padded, make([]byte, 16)...)

	clientMsgKey := crypto.MessageKey(authKey, padded, crypto.Client)
	serverMsgKey := crypto.MessageKey(authKey, padded, crypto.Server)
	out = append(out, vector{
		Name: "kdf_v2_message_key",
		A:    hexs(clientMsgKey[:]),
		B:    hexs(serverMsgKey[:]),
	})

	ck, civ := crypto.Keys(authKey, clientMsgKey, crypto.Client)
	sk, siv := crypto.Keys(authKey, clientMsgKey, crypto.Server)
	out = append(out, vector{
		Name: "kdf_v2_keys",
		A:    hexs(ck[:]),
		B:    hexs(civ[:]),
		C:    hexs(sk[:]),
	})

	authID := authKey.ID()
	auxHash := authKey.AuxHash()
	out = append(out, vector{
		Name: "auth_key_id",
		A:    hexs(authID[:]),
		B:    hexs(auxHash[:]),
	})
	_ = siv

	// Message identifiers for a fixed timestamp.
	const nowNanos = int64(1_700_000_000_123_456_789)
	out = append(out, vector{
		Name: "message_id",
		I:    int64(proto.NewMessageIDNano(nowNanos, proto.MessageFromClient)),
		I2:   int64(proto.NewMessageIDNano(nowNanos, proto.MessageFromServer)),
		I3:   int64(proto.NewMessageIDNano(nowNanos, proto.MessageServerResponse)),
	})

	// Transport framing goldens for a fixed 8-byte payload.
	payload := []byte{1, 2, 3, 4, 5, 6, 7, 8}
	out = append(out, vector{Name: "abridged_frame", A: hexs(encodeAbridged(payload))})
	out = append(out, vector{Name: "intermediate_frame", A: hexs(encodeIntermediate(payload))})

	document := map[string]any{
		"source":  "github.com/iamxvbaba/td",
		"vectors": out,
	}
	encoded, err := json.MarshalIndent(document, "", "  ")
	if err != nil {
		panic(err)
	}
	fmt.Println(string(encoded))
}

func encodeAbridged(payload []byte) []byte {
	b := &bin.Buffer{}
	b.Put([]byte{byte(len(payload) / 4)})
	b.Put(payload)
	return b.Copy()
}

func encodeIntermediate(payload []byte) []byte {
	b := &bin.Buffer{}
	b.PutInt(len(payload))
	b.Put(payload)
	return b.Copy()
}
