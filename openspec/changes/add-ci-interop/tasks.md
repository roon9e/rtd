# Tasks

## 1. External peer

- [x] 1.1 Add a (non-blocking) CI job building `mtproto-testserver` and running the `peer_gate` tests (`.github/workflows/ci.yml`)

## 2. Vectors

- [x] 2.1 Add a (non-blocking) CI job regenerating `interop/vectors.json` and failing on diff

## 3. Drop-in

- [ ] 3.1 Add a CI job running the `rtd-client` -> `rtd-edge` -> core smoke against the pinned core image

## 4. Verification

- [ ] 4.1 Verify all new jobs pass on `dev` and are required checks
- [ ] 4.2 Run the full workspace gate locally (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
