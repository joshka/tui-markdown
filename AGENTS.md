# Repository Guidelines

## Project Structure & Module Organization
This workspace ships two crates: the core renderer in `tui-markdown/` and the demo CLI in `markdown-reader/`. Library code lives in `tui-markdown/src/lib.rs`, with golden snapshots under `tui-markdown/src/snapshots/`. The CLI entry point is `markdown-reader/src/main.rs`, and `markdown-reader/TEST.md` is the sample document exercised in demos. Shared tooling files such as `Cargo.toml`, `rustfmt.toml`, and the GitHub meta docs sit at the repository root.

## Build, Test, and Development Commands
- `cargo build --workspace --all-features` builds both crates with syntax highlighting enabled.
- `cargo run -p markdown-reader -- README.md` launches the TUI reader against a target file.
- `cargo test --workspace --all-features` executes unit, snapshot, and integration tests.
- `cargo clippy --all-targets --all-features --workspace` lints the entire workspace.
- `cargo fmt --all` formats sources; add `-- --check` in CI-style validation.

## Coding Style & Naming Conventions
Code targets Rust 1.82 (edition 2021) and follows standard Rust naming (`snake_case` functions, `PascalCase` types). `rustfmt.toml` enforces grouped imports, wrapped comments, and a 100-column guide; run `cargo fmt` before review. Prefer explicit module re-exports for library APIs and keep public surfaces documented with `///` comments. Clippy must run clean before submitting changes.

## Testing Guidelines
Unit tests and fixtures live alongside implementation inside `tui-markdown/src/lib.rs` and rely on `rstest`, `indoc`, and `pretty_assertions`. Snapshot expectations are stored in `src/snapshots/`; when tests flag differences, update them with `cargo insta review` from the crate directory after verifying the output. Targeted runs such as `cargo test highlighted_code --package tui-markdown` help isolate failures. The CLI currently lacks automated tests—document any manual verification (e.g., `cargo run -p markdown-reader -- TEST.md`) in your PR.

## Commit & Pull Request Guidelines
Commits follow Conventional Commits (`<type>(<scope>): summary`), matching the existing history (e.g., `fix(renderer): correct bullet spacing`). Squash cosmetic work into logical commits that compile and test cleanly. Pull requests should describe the change, include before/after context or screenshots for UI tweaks, and link related issues. Confirm `cargo fmt`, `cargo clippy`, and `cargo test` succeed before requesting review, and call out any remaining snapshot updates or follow-ups.

## Feature Flags & Configuration
Code highlighting ships via the default `highlight-code` feature; disable it with `cargo build -p tui-markdown --no-default-features` when a lighter build is required. Docs.rs configuration enables `document-features` during documentation builds—avoid breaking these optional paths. Tracing hooks are enabled in tests; keep new instrumentation lightweight and gated behind feature flags when possible.
