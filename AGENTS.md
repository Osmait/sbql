# AGENTS.md

Instructions for coding agents working in this repository.

## Read this first

**[`CODE_STYLE.md`](CODE_STYLE.md) is the authority on how Rust is written
here.** Read it before writing or reviewing Rust in this repo. It covers error
handling, the panic lints and why they exist, public-API rules, documentation
and comment conventions, test naming, and the Cargo hygiene expected of a
change. Do not infer style from a single nearby file — several conventions here
exist because of a specific bug, and the reasoning is written down in that file.

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers architecture and the development
environment. Swift style for the macOS app lives there, not in `CODE_STYLE.md`.

## What this project is

`sbql` is a multi-platform SQL workspace: a terminal UI (Linux and macOS) and a
native macOS app, over a shared headless engine.

```
sbql-core    Library. All database, SQL, and state logic. UI-agnostic. Published to crates.io.
sbql-tui     Binary (`sbql`). ratatui + crossterm terminal client.
sbql-ffi     UniFFI bridge to Swift. publish = false.
sbql-macos   SwiftUI app. Not part of the Cargo workspace.
```

Backends: PostgreSQL, MySQL, SQLite, SQL Server, Redis, MongoDB, DynamoDB.

The core is driven by `CoreCommand` values and answers with `CoreEvent` values.
Frontends are thin. **Any change that makes `sbql-core` depend on a terminal or
UI concept is wrong**, however convenient it looks.

## Rules that are easy to get wrong

- **Never `unwrap`, `expect`, or `panic!` in shipped code.** A panic here does
  not print a stack trace — it leaves the user's shell in raw mode on the
  alternate screen. Clippy warns on all three; test code is exempt via
  `clippy.toml`. See `CODE_STYLE.md` §1.
- **Never print to stdout/stderr in library or TUI code.** Use `tracing`. While
  the alternate screen is held, a `println!` lands in the middle of the render
  and stays there. `sbql-tui/src/main.rs` is the only legitimate exception.
- **Tests that touch config must set `SBQL_CONFIG_DIR` to a temp dir** (the
  constant is `sbql_core::CONFIG_DIR_ENV`). Without it they overwrite the
  developer's real `~/.config/sbql/connections.toml`.
- **Never put a secret where it can be printed.** Types holding a password get
  a redacting manual `Debug`, not `#[derive(Debug)]`, and log lines must not
  include a value whose `Debug` you have not read. See `CODE_STYLE.md` §2.
- **Container-backed test suites must run serially.** In parallel they are flaky
  in a way that reads as a product bug.
- **Do not add `--all-targets` to the CI test step.** It runs the benchmarks,
  whose testcontainers guard outlives the Tokio runtime and panics during
  teardown. Benches are compile-checked in a separate step on purpose.
- **New workspace crates need `[lints] workspace = true`** or they silently
  inherit none of the lint configuration.

## Verifying a change

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --lib --bins --tests
cargo check --workspace --all-targets                  # benches and examples
make audit                                             # cargo-deny
```

The `cargo doc` step is not optional: `rustdoc::broken_intra_doc_links` is
`deny`, and only rustdoc evaluates it — clippy will not catch a doc link broken
by a rename.

`make help` lists the release, benchmark, and macOS targets.

Database integration tests need a running Docker daemon. Tests marked
`#[ignore]` need one too and are not run by CI.

## Writing the change up

Commits follow Conventional Commits (`feat(tui):`, `fix(core):`, `chore:`) —
`release-plz` derives versions and changelogs from them, so the prefix is
load-bearing, not decoration.

When you touch something for a non-obvious reason, put the reason in a comment
next to the code rather than only in the commit message. The comment is what the
next person reads. `CODE_STYLE.md` §5 has the examples worth imitating.
