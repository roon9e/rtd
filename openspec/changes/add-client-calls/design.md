# Design

## Context

Telegram calls negotiate a DH exchange over `phone.*` methods, then exchange encrypted signalling
(`phone.sendSignalingData`) and media over a WebRTC connection to reflector/SFU endpoints. `td`
implements V2 signalling and media with `pion/webrtc/v4`.

## Goals / Non-Goals

**Goals**

- Establish a 1:1 call and exchange audio; join a group call.
- A Rust WebRTC media path.

**Non-Goals**

- TURN relay; V3 signalling.

## Decisions

### D1. Separate signalling from media

A signaling module owns the encrypted control channel and DH; a connection module owns the
WebRTC peer connection and tracks. This mirrors `td`'s split.

### D2. Reuse the client's update path

`phone.*` updates are dispatched through `add-client-background-receive` /
`add-client-update-recovery`, not a second reader.

### D3. Choose a Rust WebRTC stack

Use a maintained Rust WebRTC crate (e.g. `webrtc`) rather than binding `pion`; pin the version
and isolate it behind the media module.

## Risks / Trade-offs

- [Media stack is large and evolving] → keep calls in its own feature/module and its own change;
  do not block other client work.
