# Project guidance

- Keep modules focused on their existing responsibilities; keep `src/main.rs` a thin entry point and put application behavior in the appropriate module.
- Keep all tests and test fixtures under `tests/`, including unit tests. Avoid unnecessary dependencies and abstractions.
- When changing behavior, reconcile the implementation, tests, and relevant documentation. Put detailed architecture and feature guidance in `docs/`; consult the relevant docs rather than duplicating them here.
- From the repository root, run `cargo fmt --check` and `cargo test`. Before release, follow the complete PTY release-gate commands and requirements in [docs/keymaps.md](docs/keymaps.md#local-verification).
