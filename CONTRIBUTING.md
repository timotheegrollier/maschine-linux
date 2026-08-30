# Contributing

## Principes

- Code propre, commenté en français ou anglais, `cargo fmt` + `cargo clippy -- -D warnings` verts.
- Pas d'alloc/lock dans le callback audio.
- Tests pour `maschine-proto` (vectors `usbmon`) et `maschine-engine` (golden JSON).
- Commits conventionnels (`feat:`, `fix:`, `docs:`, `perf:`).

## Setup

```bash
sudo dnf install alsa-lib-devel jack-audio-connection-kit-devel libusb1-devel hidapi-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Branches

- `master` : stable
- `feat/*` : features
- PRs contre `master` avec CI verte.

## Protocole USB

Voir `docs/protocol.md` et `crates/maschine-proto`. Toute nouvelle capture `usbmon` doit être ajoutée à `crates/maschine-proto/tests/vectors.rs`.

## License

GPL-3.0-or-later. Toute contribution accepte cette licence.
