# Design

## Decision record correction: `channels.toggleForum` in 225–232

Supersedes the corresponding row of `2026-11-06-bump-canonical-layer-233`. Source: `td` reference
`iamxvbaba/td` `main@1c9e6bc64` (merge of `roon9e/feat/canonical-layer-233@573428b70`,
"fix(protocol): default historical toggleForum sidebar, regenerate on LF").

### Correction

| Scenario | Superseded | Corrected |
|---|---|---|
| `channels.toggleForum` in 225–232 (profile→canonical) | reject admission | **admit and default `sidebar=false`** |

Rationale (upstream): the historical request still carries `enabled`/`tabs`, which choose between
the list and tabs layouts; defaulting `sidebar=false` preserves the old client's intent instead of
locking it out. The canonical→profile direction is unchanged (drop `sidebar`, emit `#3ff75734`).

### Scope of the change

The `td` `policy.json` is byte-identical to the previous import except for the eight
`layer-obligation/v1/required/…` resolutions for `semantic=function:channels.toggleForum`
across layers 225–232, which change `action` from `reject` to `default`. No key is added or
removed, and the schema (`layer-233.tl`) is unchanged
(`c2bbefca…`, matching the manifest).

### Verification

- `rtd-tlprofile` runtime test `toggle_forum_sidebar_defaults_in_historical_profiles`: a
  canonical 233 request round trips; a hand-built historical `#3ff75734` body is admitted by
  every layer 225–232 and yields the canonical request with `sidebar=false`.
- `rtd-codegen generate`/`verify` accept the corrected policy; the full workspace gate is green.
