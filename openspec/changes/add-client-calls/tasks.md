# Tasks

## 1. Signalling

- [x] 1.1 Add the AES-IGE signaling control channel and `CallState` (`calls.rs`)
- [x] 1.2 Add call place/answer/confirm RPCs and the DH core (`calls.rs`; reflector/STUN parsing and `phone.*` dispatch remain)

## 2. Media

- [ ] 2.1 Add a WebRTC media connection with audio tracks, `OnTrack`/`OnConnected`/`OnDisconnected`
- [x] 2.2 Add group-call join/leave with `WriteAudio`/`AudioTrack`/`AudioSSRC` and RTCP sender reports

## 3. Verification

- [x] 3.1 Verify a 1:1 call is placed and answered against a mock signalling server (`calls` tests)
- [x] 3.2 Verify a group call joins and audio flows (media test)
- [x] 3.3 Run the full workspace gate (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
