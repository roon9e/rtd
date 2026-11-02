# Spec Delta

## ADDED Requirements

### Requirement: Same-port server descriptor

The edge SHALL serve the public server descriptor on the HTTP side of the MTProto port:
`GET`/`HEAD /owpengram/server-info` SHALL return JSON with `dc_id` and `rsa_public_key_pem` (a
PKCS#1 `RSA PUBLIC KEY`), and `/owpengram/server-icon` SHALL return the identity icon or 404 when
none. Every other path SHALL return 404, and a WebSocket upgrade SHALL still be served. The
response SHALL carry an explicit `Content-Length`.

#### Scenario: A client fetches the server descriptor
- **WHEN** a client sends `GET /owpengram/server-info` on the MTProto port
- **THEN** the edge returns JSON containing the datacenter and the server's RSA public key

#### Scenario: The icon may be absent
- **WHEN** a client sends `GET /owpengram/server-icon` and no icon is configured
- **THEN** the edge returns 404, and the descriptor still advertises the key

#### Scenario: WebSocket still upgrades
- **WHEN** a client sends a WebSocket upgrade request
- **THEN** the edge upgrades it to an MTProto-over-WebSocket session
