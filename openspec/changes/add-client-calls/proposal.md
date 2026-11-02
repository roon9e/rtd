# Proposal

## Why

Voice/video calls (1:1 and group) are a headline `td` feature (`telegram/calls`, `tgcalls v2`,
`pion/webrtc`). `rtd` has the TL types but no call runtime, so a Telegram-compatible client
cannot place or join calls.

## What Changes

- 1:1 VoIP: `Client` (place/answer), `Request`/`Accept`/`IncomingCall`, DH exchange, an encrypted
  control channel, reflector/STUN endpoints, and a `pion/webrtc` media connection with
  `OnTrack`/`OnConnected`/`OnDisconnected`.
- Group calls: join/leave an SFU, `WriteAudio`/`AudioTrack`/`AudioSSRC`, RTCP sender reports and
  participant updates.
- An update dispatcher for `phone.*` signalling.

## Capabilities

### New Capabilities

- `client-calls`: 1:1 and group VoIP calls (tgcalls v2).

## Impact

- **Crates**: `rtd-client` (new `calls` module); new deps `pion`-equivalent Rust WebRTC.
- **Non-goals**: TURN relay fallback and V3 signalling (as `td`); the server-side SFU.
