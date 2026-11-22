# Contributing

Start with [AGENTS.md](AGENTS.md) (repository map, invariants, where to change
what, release steps) and [docs/STATUS.md](docs/STATUS.md) (what is done and
what is next).

## Principles

- Keep the code clean and comment in English (or French in legacy areas); `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- No allocation or locking in the audio callback.
- Add tests for `maschine-proto` (`usbmon` vectors) and `maschine-engine` (golden JSON).
- Use conventional commits (`feat:`, `fix:`, `docs:`, `perf:`).

## Setup

```bash
sudo dnf install alsa-lib-devel jack-audio-connection-kit-devel libusb1-devel hidapi-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The MSRV is **Rust 1.88**, declared in `Cargo.toml` (each crate inherits
`rust-version.workspace = true`) and enforced by the `msrv` CI job
(`cargo check --workspace --locked`). Keep contributions compatible with 1.88
and mention it in the pull request if a change requires a newer toolchain.

## Branches

- `master`: stable
- `feat/*`: features
- Open pull requests against `master` with green CI.

## USB protocol

See `docs/protocol.md` and `crates/maschine-proto`. Any new `usbmon` capture
must be added to `crates/maschine-proto/tests/vectors.rs`.

## License

GPL-3.0-or-later. Every contribution accepts this license.
