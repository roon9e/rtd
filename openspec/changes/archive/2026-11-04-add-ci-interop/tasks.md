# Tasks

## 1. External peer

- [x] 1.1 Add a manual `Interop` workflow building `mtproto-testserver` and running the `peer_gate` tests (`.github/workflows/interop.yml`)

## 2. Vectors

- [x] 2.1 Add a manual job regenerating `interop/vectors.json` and failing on diff

## 3. Drop-in

- [x] 3.1 Add a CI job running the `rtd-client` -> `rtd-edge` -> core smoke against the pinned core image (manual `drop-in` job using the published `rtd-edge` image and the gramsrv v2 stack; the edge source is no longer built here)

## 4. Verification

- [x] 4.1 (descoped) Required-check promotion is dropped: the edge moves to its own repository, so the drop-in smoke stays a manual `workflow_dispatch` job rather than a required check.
- [x] 4.2 Run the full workspace gate locally (`fmt`, `clippy -D warnings`, `test --workspace --locked`, generation freshness) and confirm all are green
