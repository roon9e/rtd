# Tasks

## 1. Same-port server descriptor

- [x] 1.1 Add an optional `ServerInfo` descriptor to `IngressConfig` and serve
  `/owpengram/server-info` (JSON) and `/owpengram/server-icon` (or 404) on the HTTP side of the
  MTProto listener, replaying the request head for a WebSocket upgrade; verify a `GET` returns the
  JSON with the RSA public key and a WebSocket upgrade still works (`ingress.rs` tests
  `server_info_json_escapes_and_carries_the_key`, `server_info_response_routes_paths`,
  `server_info_rejects_non_read_methods`; the existing WebSocket round trip still passes)
- [x] 1.2 `rtd-edge` builds the descriptor from its datacenter and RSA public key; verify against a
  running client (`service::server_info`; live: `GET` returns `dc_id:2` + the PKCS#1 key)

## 2. Verification

- [x] 2.1 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`,
  generation freshness) and confirm all are green
- [x] 2.2 Rebuild the edge and confirm `GET /owpengram/server-info` returns the key
  (`HTTP/1.1 200 OK`, `Content-Length: 513`, the JSON body), and that the end-to-end login still
  succeeds (`E2E OK`)
