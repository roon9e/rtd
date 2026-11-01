# Contributing to rtd

## Branch model

- **`main`** is the release/default branch. It is always in a releasable state.
- **`dev`** is the working branch. All development happens here.
- Releases are made by merging `dev` into `main`; `main` is not committed to directly.

CI runs on both `dev` and `main` (see `.github/workflows/ci.yml`).

## Local checks

Before opening a pull request, run the same independent checks CI runs:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

The generator is verified separately: regenerate and confirm no diff.

```console
cargo run --locked -p rtd-codegen -- generate
git diff --exit-code
```

## Conventions

- Edition 2024; the minimum supported Rust version tracks the latest stable toolchain.
- `unsafe` code is forbidden workspace-wide (`unsafe_code = "forbid"`).
- Public items must be documented (`missing_docs`); clippy warnings are denied
  (`clippy::all = deny`).
- Generated crates (`rtd-mt`, `rtd-tg`, `rtd-tlprofile`) are produced by
  `rtd-codegen` and must never be edited by hand.

## Licensing

rtd is dual-licensed as `MIT OR Apache-2.0`. By contributing you agree that your
contributions are made under the same terms. Material derived from `td`/`gotd`
retains its MIT notice in [`NOTICE`](NOTICE).

## Planning

This project is planned with [OpenSpec](https://openspec.dev). Specs and decisions
live under `openspec/`. See `openspec/config.yaml` for the project's frozen core
decisions.
