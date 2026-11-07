# Contributing

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

## Branches

- `master`: stable
- `feat/*`: features
- Open pull requests against `master` with green CI.

## USB protocol

See `docs/protocol.md` and `crates/maschine-proto`. Any new `usbmon` capture
must be added to `crates/maschine-proto/tests/vectors.rs`.

## License

GPL-3.0-or-later. Every contribution accepts this license.
