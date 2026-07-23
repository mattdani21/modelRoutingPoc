# AGENTS.md

## Cursor Cloud specific instructions

Tessera Model Gate is a single Rust service (`axum` + embedded SQLite via `rusqlite` `bundled`) that also serves the static dashboard in `web/`. It is a self-contained, loopback-only POC — no external database, cache, or queue is needed to run and test it.

### Toolchain gotcha (important)
The crate uses Rust `edition = "2024"`, which requires Rust >= 1.85. The base image may default to an older toolchain (e.g. 1.83), which fails to compile. Use the `stable` toolchain: `rustup default stable`. The update script already installs/sets this.

### Run / test / build
Standard commands are documented in `README.md` and mirrored in `.github/workflows/ci.yml`. In short:
- Run the service: `cargo run` (binds `127.0.0.1:8080`; open `http://127.0.0.1:8080`). Flags: `--bind`, `--benchmarks`, `--models`, `--database`.
- Rust tests: `cargo test --all-targets`; lint/check: `cargo check --all-targets`.
- Frontend (vanilla JS, no bundler/npm): `node --check web/logic.js`, `node --check web/app.js`, `node tests/web_logic.test.js`, `node tests/html_contract.test.js`.

### Behavior notes
- The dashboard defaults to demo mode (`demo: true`) and needs no secrets or model endpoints; demo runs are always `demo_only` and never become `eligible` by design.
- Live runs require an OpenAI-compatible endpoint per `config/models.yaml` and, for the hosted route, the API key named by `api_key_env` (e.g. `PREMIUM_API_KEY`). Do not put keys in YAML.
- The SQLite file (default `model-gate.db`) and `ledger-*.csv` are gitignored and created at runtime.
