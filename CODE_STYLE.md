# sbql Rust code style

How Rust is written in this repository, and why. This is the reference for
humans and for coding agents alike — `AGENTS.md` and `CLAUDE.md` point here.

Each rule is tagged with how it is enforced:

| Tag | Meaning |
| --- | --- |
| **[tool]** | A lint, `rustfmt`, or CI catches it. You cannot merge past it. |
| **[review]** | Convention. Nothing catches it automatically; reviewers do. |
| **[gap]** | Agreed direction, not yet enforced. Follow it in new code; do not churn old code to match. |

The house style is deliberately close to what `ripgrep`, `jj`, `ratatui`, and
Astral's `uv`/`ruff` do — those are the projects this file was calibrated
against. Where we deviate, the deviation is explained.

---

## 1. Formatting and lints

**[tool]** Format with `cargo fmt`. Default `rustfmt` settings, no
`rustfmt.toml` overrides. Default settings are what every Rust reader and every
tool already expects; a custom config buys nothing and costs everyone a surprise.

**[tool]** `cargo clippy --workspace --all-targets` must be clean. Zero
warnings, no exceptions.

**[review]** Silence a lint at the narrowest possible scope, and say why:

```rust
// The keyring is mocked in this test, so the failure path is the point.
#[allow(clippy::expect_used)]
let cwd = std::env::current_dir().expect("a current directory");
```

A bare `#[allow(...)]` with no comment is a review comment waiting to happen. A
crate-level `#![allow(...)]` needs a much stronger argument than a local one —
prefer fixing the code.

### The panic lints

`Cargo.toml` sets, for the whole workspace:

```toml
[workspace.lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
```

and `clippy.toml` stands them down inside `#[cfg(test)]`. Every crate opts in
with `[lints] workspace = true` — **if you add a crate to the workspace, add
that stanza or it silently inherits nothing.**

This is not generic hygiene. A panic in `sbql` is not a stack trace on stderr:
it is a garbled shell, because the terminal is still in raw mode and the
alternate screen. `sbql-tui/src/tui.rs` installs a panic hook that restores the
terminal, but that is damage control. These lints are what stop the panic
being written in the first place.

Not enabled: `indexing_slicing`. `x[i]` panics as readily as `unwrap` while
reading as though it cannot, but the diagram canvas indexes heavily and would
have to be reworked first. **[gap]**

---

## 2. Errors

### Two error types, on purpose

`sbql-core/src/error.rs` is the model to copy.

- **`SbqlError`** — the internal one. `thiserror`, `#[from]`-friendly, returned
  by every fallible function in the crate. Rich, and free to wrap driver types.
- **`CoreError`** — what leaves the crate. Flat, `Clone`, and carries a
  machine-readable [`ErrorKind`] plus a [`Severity`], a one-line message for a
  status bar, and the rest of the cause chain in `detail`.

**[review]** The rule this encodes: *a client must never have to parse prose to
decide what to do.* `CoreError.kind` exists so the UI can branch on "should I
suggest connecting?" without substring-matching the message. Add a variant to
`ErrorKind` only when a client would genuinely act differently on it — variants
that all get handled the same way are noise.

**[review]** Do not throw away the cause chain. `sqlx::Error`'s own `Display`
says "error returned from database" and keeps the server's actual complaint in
its `source()`. `source_chain()` in `error.rs` flattens the chain and skips
causes already present in the message above them. Anything that stringifies an
error with a bare `to_string()` loses the only part worth reading.

### thiserror vs anyhow

**[review]** `thiserror` in libraries (`sbql-core`, `sbql-ffi`), so callers can
match. `anyhow` only at the top of a binary, where the next step is printing and
exiting. Do not let `anyhow::Error` into a public signature.

### Error text

**[review]** Lowercase, no trailing period, no "failed to" prefix on every
variant — the display chain composes, so `"Database error: {0}"` followed by
`"connection refused"` should read as one sentence. Errors name the thing that
went wrong (`"No saved password for '{0}'"`), not the function that noticed.

---

## 3. Public API

`sbql-core` is published to crates.io. `sbql-tui` and `sbql-ffi` are
`publish = false` and their surface is internal — but `sbql-ffi`'s surface *is*
the Swift API, so it gets the same care for a different reason.

**[review]** Everything below is from the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html);
the ids are theirs, so you can look up the rationale.

- **C-COMMON-TRAITS** — public types derive `Debug`, and `Clone`/`Copy`/`PartialEq`
  where they are values. `Debug` is non-negotiable: `#[derive(Debug)]` on a public
  type is free, and its absence poisons every caller's own `derive(Debug)`.
  *Known gap: `Core` in `sbql-core/src/lib.rs` derives `Clone, Default` but not
  `Debug`.* **[gap]**
- **C-NEWTYPE / C-CUSTOM-TYPE** — arguments convey meaning through types, not
  `bool`. `SortDirection` and `Severity` are enums rather than flags for this
  reason. A function taking two `bool`s is a bug that has not happened yet.
- **C-STRUCT-PRIVATE** — prefer private fields with accessors for types that
  have invariants. Plain data carriers crossing the UI boundary (`CoreError`,
  `QueryResult`, `ConnectionConfig`) keep public fields deliberately: they are
  messages, not objects, and an accessor per field would be ceremony.
- **Future proofing** — public enums a client matches on get
  `#[non_exhaustive]`, so adding a variant is not a breaking change.
  `ErrorKind` has it. `SbqlError`, `Severity`, `CoreCommand`, and `CoreEvent`
  do not, and should. **[gap]**
- **C-METADATA** — `sbql-core`'s `[package]` should carry `readme`, `keywords`,
  `categories`, and `rust-version` on top of the `description`/`repository` it
  already has. **[gap]**

**[review]** `pub` means "part of the contract". If an item is only needed
inside the crate, it is `pub(crate)`. We plan to make the compiler enforce this
with `unreachable_pub`. **[gap]**

---

## 4. Documentation

The repo is already good at this — 48 modules carry `//!` docs. Keep it that way.

**[review]** Every module starts with a `//!` block that says what the module is
*for*, not what it contains. Compare:

```rust
//! Errors, in two shapes.
//!
//! [`SbqlError`] is the internal one: rich, `#[from]`-friendly, and what every
//! fallible function in this crate returns.
```

against a hypothetical "Error types for sbql-core." The first one tells you
which of the two to reach for.

**[review]** Link types with intra-doc links (`` [`CoreEvent::Error`] ``), not
backticked prose. They are checked at doc-build time; prose is not. We plan to
turn `rustdoc::broken_intra_doc_links` into a hard error. **[gap]**

**[review]** Document the *interesting* half. `pub fn is_warning(&self) -> bool`
needs nothing. A function whose behaviour surprised someone once needs a
paragraph about why it behaves that way — see `FetchTotalCount` in
`sbql-core/src/lib.rs`, whose doc explains that counting is a separate command
because folding it into page 0 made every first page wait on a `COUNT(*)`.

**[gap]** `#![warn(missing_docs)]` on `sbql-core`, and `missing_errors_doc` for
its public `Result`-returning functions. Currently ~48 public functions would
warn; this is a staged cleanup, not a flag day.

---

## 5. Comments

This is the repo's strongest existing habit and the one most worth protecting.

**[review]** Comments explain **why**, and the best ones name the failure that
motivated the code:

```rust
/// `pk` carries every `(column, value)` component of the row's primary
/// key. Sending only the first component of a composite key once turned
/// "update this row" into "update every row sharing that component".
```

```toml
# `default-features = false` has to live here: Cargo ignores a member's
# `default-features = false` when the dependency is inherited from the
# workspace, so members opt back in via their own `keyring` feature.
```

Both of these save the next person a debugging session. A comment that
restates the code (`// increment i`) does not, and should be deleted.

**[review]** Non-obvious *absences* get comments too — see the note in
`Cargo.toml` on why `indexing_slicing` is off, and in `pr-tests.yml` on why CI
does not use `--all-targets` for `cargo test`. "Why isn't this here?" is a
question the code cannot answer on its own.

**[review]** Comments are in English, matching the rest of the codebase.

---

## 6. Naming

**[tool]** `snake_case` / `CamelCase` / `SCREAMING_SNAKE` per RFC 430; the
compiler enforces it.

**[review]** Conversions follow **C-CONV**: `as_` is a free borrow, `to_`
allocates or is expensive, `into_` consumes. Do not use `into_` for something
that borrows.

**[review]** Prefer `From`/`TryFrom` over inherent `to_x()` constructors —
`sbql-ffi/src/convert.rs` does this throughout, which is why `?` works across
the FFI boundary without adapters.

**[review]** Word order is consistent across the crate (**C-WORD-ORDER**):
`ConnectionConfig`, `ConnectionDraft`, `ConnectionField` — noun first. Do not
introduce `DraftConnection` alongside them.

---

## 7. Panics, printing, and the terminal

**[tool]** Covered by the panic lints in §1.

**[gap]** `print_stdout`, `print_stderr`, and `dbg_macro` should be `warn`.
This matters more here than in a normal CLI: while the TUI holds the alternate
screen, anything written to stdout lands in the middle of the render and stays
there. `uv` and `ruff` both deny these; for us it is a correctness lint, not a
tidiness one.

**[review]** Diagnostics go through `tracing`, never `println!`. The repo has 41
`tracing` call sites and 7 prints; of those prints, the ones in
`sbql-tui/src/main.rs` are legitimate (they run before the TUI takes the screen
and after it gives it back), and `sbql-ffi/src/lib.rs:220` is not — it writes to
a stderr no macOS app user will ever see, and should be `tracing::warn!`.

---

## 8. Async and concurrency

**[review]** `sbql-core` is UI-agnostic and stays that way. The `Core` type is
driven by `CoreCommand` values and answers with `CoreEvent` values; it never
knows what is rendering. Any change that makes core depend on `ratatui`,
`crossterm`, or a terminal concept is wrong regardless of how convenient it is.

**[review]** Hold locks for the shortest possible span. Bind the guard, use it,
drop it — do not `await` while holding one. Clippy's
`significant_drop_tightening` flags the current offenders. **[gap]**

**[review]** Long work does not block the event loop. `sbql-tui/src/worker.rs`
owns the pattern: the UI thread sends a command and keeps rendering; the result
arrives as an event.

---

## 9. Performance

**[review]** Clone deliberately, not reflexively. `a = b.clone()` should be
`a.clone_from(&b)` when `a` already holds an allocation worth reusing —
`connection_spec.rs:433-437` and `discovery.rs:429` are current examples.

**[review]** Build strings with `write!` into the buffer, not
`s.push_str(&format!(...))`, which allocates a throwaway `String` per push.
Current sites: `config.rs:687`, `query.rs:1827`, `query_builder.rs:208`.

**[review]** Use inline format captures — `format!("{name}")`, not
`format!("{}", name)`. Shorter, and it cannot get the argument order wrong. The
codebase already does this in most places; ~40 sites still do not. **[gap]**

**[review]** Measure before optimising. `sbql-core/benches/` has Criterion
benches and `make profile-memory` runs a dhat heap profile. A perf claim in a PR
description should have a number next to it.

---

## 10. Tests

**[review]** Test names are sentences that state the claim, not labels:

```rust
fn an_unreachable_server_is_a_connection_problem_not_a_query_one()
fn a_cause_already_in_the_message_is_not_repeated()
```

`fn test_error()` tells a failing CI log nothing. These names mean the test
output *is* the spec. This is the convention throughout `error.rs` and it should
spread, not fade.

**[review]** Non-obvious tests get a doc comment saying what invariant is at
stake — `/// The whole point of `detail`: the cause survives instead of being
flattened away by `to_string()`.`

**[review]** Unit tests live in `#[cfg(test)] mod tests` next to the code.
Integration tests that need a database live in `sbql-core/tests/` and use
testcontainers.

**Two hazards, both learned the hard way:**

- Tests that touch config **must** point `CONFIG_DIR_ENV` at a temp directory.
  Without it they write the developer's real `~/.config/sbql/connections.toml`.
  `sbql-ffi`'s dev-dependencies carry a comment about exactly this.
- Container-backed suites run serially. Running them in parallel is flaky in a
  way that looks like a product bug.

**[review]** CI does not run `cargo test --all-targets` — that also *runs* the
benchmarks, whose testcontainers guard drops after the Tokio runtime is gone
("there is no reactor running"). Benches are covered by a separate
`cargo check --all-targets` step so they cannot rot. Do not "fix" this by adding
`--all-targets` to the test step.

---

## 11. Cargo hygiene

**[review]** Dependency versions live in `[workspace.dependencies]`; members
write `foo.workspace = true`. One version, one place.

**[review]** Feature flags get a comment explaining what breaks without them.
The `keyring` block in `sbql-core/Cargo.toml` is the standard: it explains that
enabling a backend feature for the wrong OS makes `keyring` fall back to an
in-memory mock, so passwords silently do not survive a restart. That is not
inferable from the code.

**[review]** A `--no-default-features` build is a supported configuration, and
CI tests it. If you gate something behind `keyring`, make sure the ungated path
still compiles and still tells the user the truth about what was saved.

**[gap]** Missing and worth adding, in rough priority order:

1. **`fmt` + `clippy` jobs in `pr-tests.yml`.** `CONTRIBUTING.md` claims a zero
   warnings policy and nothing in CI enforces it. This is the single largest gap.
2. **`rust-toolchain.toml` + `rust-version`** — pin the toolchain so CI and
   laptops agree, and declare an MSRV.
3. **`[profile.release]`** — we ship prebuilt binaries and have no release
   profile at all. `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`
   is the conservative starting point (`helix` and `jj` ship roughly this).
4. **`deny.toml` + a `cargo-deny` CI job** — advisories, licenses, and banned
   crates. We link five database drivers and an SSH stack; nobody is watching
   RustSec by hand.
5. **`unsafe_code = "forbid"`** on `sbql-core` and `sbql-tui`. There is
   currently zero `unsafe` in the workspace, so this is free today and only gets
   more expensive to adopt later. `sbql-ffi` cannot forbid it — `uniffi`'s macros
   generate `unsafe` — so it stays at `deny` with documented exceptions there.

---

## 12. Proposed lint block

The direction §1–§11 point at, as a single diff to `Cargo.toml`. Adopt in
stages: each line that is added must be clean before the next lands, so `main`
is never yellow.

```toml
[workspace.lints.rust]
unsafe_code = "warn"          # forbid on core/tui once ffi is carved out
unreachable_pub = "warn"
let_underscore_drop = "warn"

[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"

[workspace.lints.clippy]
# Already in place — see §1.
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"

# A print during the alternate screen corrupts the render. See §7.
print_stdout = "warn"
print_stderr = "warn"
dbg_macro = "warn"

# Correctness and cost, cheap to fix, no judgement calls.
assigning_clones = "warn"
format_push_string = "warn"
implicit_clone = "warn"
redundant_clone = "warn"
uninlined_format_args = "warn"

# Readability, matching jj / ratatui.
manual_let_else = "warn"
semicolon_if_nothing_returned = "warn"
single_char_pattern = "warn"
unnested_or_patterns = "warn"
use_self = "warn"
```

Measured against the current tree (`cargo clippy -- -W clippy::pedantic
-W clippy::nursery`), the largest remaining pedantic categories are
`use_self` (~118 sites), `doc_markdown` (~53), `missing_errors_doc` (~48),
`must_use_candidate` (~40), and `uninlined_format_args` (~40).

Deliberately **not** adopted, with reasons:

| Lint | Why not |
| --- | --- |
| `pedantic` wholesale | ~500 warnings today. Adopt the useful members individually. |
| `option_if_let_else` | Nursery, and its suggestion is regularly less readable than the original. |
| `redundant_pub_crate` | ~28 sites, all cosmetic; fights `unreachable_pub`. |
| `must_use_candidate` | Would put `#[must_use]` on ~40 getters for little gain. Apply it by hand to builder methods that return `Self`. |
| `too_many_lines` | Real signal, but 31 hits means it is a refactoring backlog, not a lint. |
| `cast_possible_truncation` | The diagram canvas casts between screen and model coordinates constantly. Revisit with `indexing_slicing`. |

---

## 13. Before you open a PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib --bins --tests
cargo check --workspace --all-targets          # benches and examples
cargo test -p sbql-core --no-default-features --lib   # the keyring-free path
```

Then read your own diff and ask: *does every non-obvious line say why?*

---

## References

Calibrated against:

- [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- [`astral-sh/uv`](https://github.com/astral-sh/uv/blob/main/Cargo.toml) and
  [`astral-sh/ruff`](https://github.com/astral-sh/ruff/blob/main/Cargo.toml) —
  pedantic-with-exceptions, plus the `print_stdout`/`dbg_macro` restriction set
- [`jj-vcs/jj`](https://github.com/jj-vcs/jj/blob/main/Cargo.toml) — hand-picked
  clippy list and declared MSRV
- [`ratatui/ratatui`](https://github.com/ratatui/ratatui/blob/main/Cargo.toml) —
  `unsafe_code = "forbid"`, `use_self`, `missing_const_for_fn`
- [`helix-editor/helix`](https://github.com/helix-editor/helix/blob/master/Cargo.toml) —
  release profile for a shipped TUI binary
- [Clippy lint index](https://doc.rust-lang.org/stable/clippy/lints.html) and
  [rustdoc lints](https://doc.rust-lang.org/rustdoc/lints.html)
