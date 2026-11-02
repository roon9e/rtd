# Design

## Context

`rtd_exchange::server_exchange` read the client's `p_q_inner_data_dc`/`p_q_inner_data_temp_dc`
and required its `dc` to equal `ServerConfig.dc` (`check_dc`). The reference edge
(`gramsrv` v2, `public.strict_dc_check: false`) accepts every label because one physical backend
serves all of them. Decrypting a live capture proved the client asks for DC 3/4 and only the DC-2
attempt reached `dh_gen_ok`.

## Goals / Non-Goals

**Goals**

- Complete the handshake for a client that selects any DC label, matching the reference edge.
- Keep the strict behaviour available and tested.

**Non-Goals**

- Real multi-DC routing. This change does **not** implement it; it only stops a single-backend
  deployment from rejecting foreign labels.

## Decisions

### D1. A `strict_dc` flag, defaulted off at the edge

`ServerConfig` gains `strict_dc: bool`; the exchange calls `accept_dc`, which compares the label
only when strict. Tests keep `strict_dc: true`; `rtd-edge` passes `false`. This mirrors the
reference exactly.

### D2. Documented as a workaround for multi-DC

The flag is a **deployment-mode** switch, not a substitute for multi-DC. When a real multi-DC
runtime lands, the DC label selects the backend and is part of the key and session identity: the
runtime must map the label to a backend (or refuse an unknown one), not serve it locally. A
multi-server client can hold many accounts and may use labels far above 5, so the map must be
data-driven. The `multi-dc` spec is the place to record this; the flag here must not be read as
"any label is this DC" in that future.
