# sbql Rust code style

How Rust is written in this repository, and why. This is the reference for
humans and for coding agents alike — `AGENTS.md` and `CLAUDE.md` point here.

Each rule is tagged with how it is enforced:

| Tag | Meaning |
| --- | --- |
| **[tool]** | A lint, `rustfmt`, `cargo-deny`, or CI catches it. You cannot merge past it. |
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
warnings, no exceptions. The `lint` job in `.github/workflows/pr-tests.yml`
enforces it, with and without the `keyring` feature, and runs `cargo doc` with
`RUSTDOCFLAGS=-D warnings` on top.

The full policy lives in `[workspace.lints]` in the root `Cargo.toml`, with a
comment on every block explaining what it is for. §12 lists what is deliberately
*not* in it.

**[review]** Silence a lint at the narrowest possible scope, and say why:

```rust
// The keyring is mocked in this test, so the failure path is the point.
#[allow(clippy::expect_used)]
let cwd = std::env::current_dir().expect("a current directory");
```

A bare `#[allow(...)]` with no comment is a review comment waiting to happen.

There are 14 `#[allow]`s in the workspace. Adopting this whole lint policy
added exactly **four** of them: two `clippy::print_stdout` on `#[ignore]`d
discovery tests whose `--nocapture` output *is* their product, and two
`clippy::print_stderr` in `sbql-tui/src/main.rs` for the prints that run
outside the alternate screen. Each carries a one-line reason. The other ten
pre-date this policy — mostly `expect_used` in tests and
`field_reassign_with_default`. Keep the total small enough to audit by eye;
`grep -rn '#\[allow' --include='*.rs'` should stay readable in one screen.

A crate-level `#![allow(...)]` needs a much stronger argument than a local one.
There are none, and adding one should be a conversation.

### The panic lints

`Cargo.toml` sets, for the whole workspace:

```toml
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

## 2. Secrets

sbql holds database passwords, SSH passwords, and scraped Docker credentials.
Three separate near-misses in this codebase were all the same mistake: a secret
escaping through a channel nobody thought of as output.

**[review]** **A type that can hold a secret does not get `#[derive(Debug)]`.**
Write a manual `Debug` that redacts the field. `ConnectionDraft` (which holds a
plaintext password while the user types it) and `DiscoveredCredentials` (which
exists *specifically* so scraped passwords never travel inside a logged
`CoreEvent`) both derived `Debug` and both leaked. They now redact, and there
are tests asserting it. Write that test — the derive is one careless keystroke
away from coming back.

**[review]** **Do not log a value whose `Debug` you have not read.**
`sbql-tui`'s `action::send_command` deliberately logs that a send failed without
logging *what* was sent, because `CoreCommand::SaveConnection` carries a
password. "Log the thing you were doing" is a good default that is wrong here.

**[review]** A secret written to the OS credential store must be written on the
same side of validation as the config it belongs to, and deleted with it. Both
halves were violated by the SSH password until recently; §14.2 records what that
cost and how it was closed.

---

## 3. Errors

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
that would all get handled the same way are noise.

**[review]** Do not throw away the cause chain. `sqlx::Error`'s own `Display`
says "error returned from database" and keeps the server's actual complaint in
its `source()`. `source_chain()` in `error.rs` flattens the chain and skips
causes already present in the message above them. Anything that stringifies an
error with a bare `to_string()` loses the only part worth reading.

**[review]** An operation that half-succeeded is a `Severity::Warning`, not an
error. Saving a connection whose password the keyring refuses still saves the
connection; reporting that as a failure tells the user their work was lost when
it was not.

### thiserror vs anyhow

**[review]** `thiserror` in libraries (`sbql-core`, `sbql-ffi`), so callers can
match. `anyhow` only at the top of a binary, where the next step is printing and
exiting. Do not let `anyhow::Error` into a public signature.

### Error text

**[review]** Lowercase, no trailing period, no "failed to" prefix on every
variant — the display chain composes, so `"Database error: {0}"` followed by
`"connection refused"` should read as one sentence. Errors name the thing that
went wrong (`"No saved password for '{0}'"`), not the function that noticed.

**[review]** Never swallow an error silently. Three swallowed errors were found
by turning on `let_underscore_drop`, and each had a user-visible symptom nobody
could explain: a password left in the keyring after deleting its connection, an
SSH tunnel copy failure that read as a flaky database, and `connections.toml.tmp.*`
files accumulating forever. If a failure is genuinely acceptable, log it at
`tracing::debug!` and say in a comment why it is acceptable.

---

## 4. Public API

`sbql-core` is published to crates.io. `sbql-tui` and `sbql-ffi` are
`publish = false` and their surface is internal — but `sbql-ffi`'s surface *is*
the Swift API, so it gets the same care for a different reason.

**[review]** Everything below is from the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html);
the ids are theirs, so you can look up the rationale.

- **C-COMMON-TRAITS** — public types derive `Debug`, and `Clone`/`Copy`/`PartialEq`
  where they are values. `Debug` is non-negotiable: its absence poisons every
  caller's own `derive(Debug)`. The exception is §2 — a type holding a secret
  gets a redacting manual impl, never no impl at all.
- **C-NEWTYPE / C-CUSTOM-TYPE** — arguments convey meaning through types, not
  `bool`. `SortDirection` and `Severity` are enums rather than flags for this
  reason. A function taking two `bool`s is a bug that has not happened yet.
- **C-STRUCT-PRIVATE** — prefer private fields with accessors for types that
  have invariants. Plain data carriers crossing the UI boundary (`CoreError`,
  `QueryResult`, `ConnectionConfig`) keep public fields deliberately: they are
  messages, not objects, and an accessor per field would be ceremony.
- **C-METADATA** — `sbql-core`'s `[package]` carries `readme`, `keywords`,
  `categories`, `homepage`, and `rust-version.workspace = true`. That last one
  is easy to forget and silent when missing: declaring `rust-version` in
  `[workspace.package]` does nothing for a member that never inherits it.

**[tool]** `unreachable_pub` is on. `pub` means "part of this crate's contract";
anything reachable only from inside is `pub(crate)`.

### Dead code, and why the compiler cannot find it here

**[review]** `dead_code` reports nothing in this workspace, and that is not the
same as there being none. It stops at three boundaries:

- **`sbql-core` is a library.** rustc assumes every `pub` item has a downstream
  consumer. Its only consumers are in this repo, so the question is answerable
  by cross-referencing `sbql-tui`, `sbql-ffi`, and core's own `tests/`,
  `benches/`, `examples/`.
- **`sbql-ffi` exports to Swift.** The caller is not Rust. `sbql-macos` is in
  this repo, so the same cross-reference works — remembering that uniffi
  renames `snake_case` to `camelCase`.
- **Enum variants cross crates.** `CoreCommand` is built by the frontends and
  handled by core; `CoreEvent` is the reverse. A command nobody sends is dead
  and nothing flags it.

**[review]** Three traps, each of which produced a false positive the one time
this was swept properly:

1. **A type in a public signature is public API even if no consumer names it.**
   `ValidationError` looked unreferenced; it is what `ConnectionDraft::build()`
   returns, and the TUI calls that. A grep cannot see it.
2. **A field held for its `Drop` looks exactly like a dead field.**
   `SbqlEngine.runtime` is never read, and deleting it would shut down the
   Tokio runtime under every async FFI call. Any `#[allow(dead_code)]` on a
   field must carry a comment saying which of the two it is.
3. **`SbqlError`'s variants are unused by every frontend on purpose.** That is
   the two-error design in §3 working, not a finding.

**[review]** When something is used only by tests, prefer finding its missing
production caller over deleting it. `DiscoverySource::is_here` had test-only
callers *and* a hand-inlined copy of its predicate in the discovery sort — the
fix was to call it, not to remove it.

**[tool]** `cargo machete` runs in CI and via `make audit`. It found three
unused dependencies the first time it ran. It matches on the crate name in the
source, so a dependency used only through a macro can false-positive; the fix
is an `ignored` entry in that crate's `[package.metadata.cargo-machete]`.

### `#[non_exhaustive]`, and where it is wrong

**[review]** `SbqlError` and `Severity` are `#[non_exhaustive]`. Both are things
a downstream client inspects but should never assume it has seen all of.

**`CoreCommand` and `CoreEvent` deliberately are not, and this is not an
oversight.** `sbql-tui` and `sbql-ffi` match on `CoreEvent` in 111 places, and
exhaustive matching there is the point: adding an event *should* break every
frontend until it decides what to do about it. `#[non_exhaustive]` would replace
that compiler-enforced checklist with wildcard arms that swallow new events in
silence. The comment on the type says so; do not "fix" it.

The general rule: `#[non_exhaustive]` protects *foreign* clients from your
churn. When the only clients are in this repo and you *want* the churn to be
loud, it is the wrong tool.

---

## 5. Documentation

**[tool]** `sbql-core` sets `#![warn(missing_docs)]` and
`#![warn(clippy::missing_errors_doc)]`. `rustdoc::broken_intra_doc_links` is
`deny` workspace-wide, and CI runs `cargo doc` with `-D warnings` — a link that
stops resolving after a rename fails the build. It caught one on the first run.

**[review]** **If the only doc you could write would restate the item's name,
that is evidence the item should not be `pub`.** This is the most useful rule in
this file. Turning on `missing_docs` produced ~137 warnings in `sbql-core`;
clearing them resulted in 24 items and 4 whole modules becoming `pub(crate)` and
only ~60 items actually getting documentation. The public surface got smaller
and the docs got better. Filler docs are worse than the warning — they cost
the same to read as a real one and teach nothing.

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

**[review]** Document the *interesting* half. `pub fn is_warning(&self) -> bool`
needs nothing. A function whose behaviour surprised someone once needs a
paragraph about why it behaves that way — see `FetchTotalCount` in
`sbql-core/src/lib.rs`, whose doc explains that counting is a separate command
because folding it into page 0 made every first page wait on a `COUNT(*)`.

**[review]** `# Errors` sections say what the caller can *do* about a failure,
not which enum variants exist. If there is nothing to say beyond "it returns
`SbqlError`", that is a sign the function is internal.

---

## 6. Comments

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
`Cargo.toml` on why `indexing_slicing` is off, why `panic = "abort"` is not in
the release profile, and in `pr-tests.yml` on why the test job does not use
`--all-targets`. "Why isn't this here?" is a question the code cannot answer on
its own.

**[review]** Comments are in English, matching the rest of the codebase.

---

## 7. Naming

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

## 8. Panics, printing, and the terminal

**[tool]** Covered by the panic lints in §1, and by `print_stdout`,
`print_stderr`, and `dbg_macro`.

The print lints matter more here than in a normal CLI: while the TUI holds the
alternate screen, anything written to stdout lands in the middle of the render
and stays there until a full redraw. `uv` and `ruff` both deny these; for us it
is a correctness lint, not a tidiness one.

**[review]** Diagnostics go through `tracing`, never `println!`. The only
exceptions are the two sites in `sbql-tui/src/main.rs` that run before the TUI
takes the screen and after it gives it back — both carry a local `#[allow]`
saying exactly that.

**[gap]** `tracing` from `sbql-ffi` currently goes nowhere: `sbql-tui/src/main.rs`
installs a subscriber, but nothing on the macOS path does. Until that is fixed,
a `tracing::warn!` in the FFI is routable but not recorded. See §11.

---

## 9. Async and concurrency

**[review]** `sbql-core` is UI-agnostic and stays that way. The `Core` type is
driven by `CoreCommand` values and answers with `CoreEvent` values; it never
knows what is rendering. Any change that makes core depend on `ratatui`,
`crossterm`, or a terminal concept is wrong regardless of how convenient it is.

**[review]** Hold locks for the shortest possible span. Bind the guard, use it,
drop it — do not `await` while holding one. Clippy's
`significant_drop_tightening` flags the current offenders. **[gap]**

**[tool]** `let_underscore_drop` is on, which is how a dropped-immediately lock
guard gets caught. **[review]** When you *do* mean to discard a value, write
`drop(x);` so it reads as deliberate rather than as an oversight.

**[review]** Long work does not block the event loop. `sbql-tui/src/worker.rs`
owns the pattern: the UI thread sends a command and keeps rendering; the result
arrives as an event.

---

## 10. Performance

**[tool]** `assigning_clones`, `format_push_string`, `implicit_clone`,
`redundant_clone`, and `uninlined_format_args` are all on. Between them they
cover the cheap wins: `clone_from` reuses the destination's allocation, `write!`
skips a throwaway `String` per push, and inline captures cannot get the argument
order wrong.

**[review]** Clone deliberately, not reflexively. The lints catch the mechanical
cases; they do not catch cloning a whole `QueryResult` to read one field.

**[review]** Watch for work done per frame and thrown away. The diagram view
built a `format!` string every single frame and discarded it with
`let _ = search_text;`. Nothing flagged it as expensive — the only reason it
surfaced was that the discard itself became a lint.

**[review]** Measure before optimising. `sbql-core/benches/` has Criterion
benches, and `make profile-memory` runs a dhat heap profile against the
`profiling` profile, which inherits `release` so what you profile is what ships.
A perf claim in a PR description should have a number next to it.

---

## 11. Tests

**[review]** Test names are sentences that state the claim, not labels:

```rust
fn an_unreachable_server_is_a_connection_problem_not_a_query_one()
fn a_cause_already_in_the_message_is_not_repeated()
```

`fn test_error()` tells a failing CI log nothing. These names mean the test
output *is* the spec. This is the convention throughout `error.rs` and it should
spread, not fade.

**[review]** Non-obvious tests get a doc comment saying what invariant is at
stake — ``/// The whole point of `detail`: the cause survives instead of being
flattened away by `to_string()`.``

**[review]** Unit tests live in `#[cfg(test)] mod tests` next to the code.
Integration tests that need a database live in `sbql-core/tests/` and use
testcontainers.

### Code whose failure mode is silent acceptance needs a live test

**[review]** Some code cannot be tested by mocking, because the broken version
and the working version both compile and both pass every other test.
`SshHandler::check_server_key` is the case that forced this rule: stubbed to
`Ok(true)` it accepts any host key, passes the entire suite, and hands the SSH
password and all database traffic to whoever answers on the wire. Only a live
handshake tells the two apart.

`tunnel.rs` therefore carries an `#[ignore]`d test that spawns a real `sshd`,
learns a host key, restarts the server under a *different* key, and asserts the
reconnection is refused. Run it after any `russh` bump — the doc comment has the
invocation.

**[review]** **A test like that needs a negative control, and you should run
it.** Deliberately break the thing being protected and confirm the test fails;
otherwise you have verified that the test passes, not that it detects anything.
Note that neutering the `KeyChanged` arm makes this particular test *hang*
rather than fail an assertion — decisive, but slower than it should be. Do not
mistake a long-running test here for a passing one.

**[review]** A test that needs `HOME` redirected must **assert** that it was,
not call `set_var`. Learning a host key and then deliberately invalidating it
must never be able to reach a developer's real `~/.ssh/known_hosts`.

**Two hazards, both learned the hard way:**

- Tests that touch config **must** point `SBQL_CONFIG_DIR` at a temp directory.
  Without it they write the developer's real `~/.config/sbql/connections.toml`.
  `sbql-ffi`'s dev-dependencies carry a comment about exactly this.
- Container-backed suites run serially. Running them in parallel is flaky in a
  way that looks like a product bug.

**[review]** CI does not run `cargo test --all-targets` — that also *runs* the
benchmarks, whose testcontainers guard drops after the Tokio runtime is gone
("there is no reactor running"). Benches are covered by a separate
`cargo check --all-targets` step so they cannot rot. Do not "fix" this by adding
`--all-targets` to the test step. (The `lint` job *does* use `--all-targets`,
because clippy only builds them.)

---

## 12. Dependencies and Cargo hygiene

**[review]** Dependency versions live in `[workspace.dependencies]`; members
write `foo.workspace = true`. One version, one place.

**[review]** Feature flags get a comment explaining what breaks without them.
The `keyring` block in `sbql-core/Cargo.toml` is the standard: it explains that
enabling a backend feature for the wrong OS makes `keyring` fall back to an
in-memory mock, so passwords silently do not survive a restart. That is not
inferable from the code.

**[review]** A `--no-default-features` build is a supported configuration, and
CI lints, builds, and tests it. If you gate something behind `keyring`, make
sure the ungated path still compiles and still tells the user the truth about
what was saved.

**[tool]** `cargo deny check` runs on every PR and covers advisories, licences,
bans, and sources. `make audit` runs the same thing locally. Config and the
reasoning behind every exception are in `deny.toml`.

**[review]** **Do not add an advisory to the ignore list if a `cargo update`
fixes it.** Two entries were added and removed within a day for exactly this
reason. A stale ignore is worse than no audit: it silently covers the
regression when it comes back. An ignore needs the advisory id, why it is not
exploitable *here*, and what would make it removable.

**[review]** Licence exceptions go per-crate, not into the global allow-list, so
that a *new* dependency under the same licence still trips the check.

---

## 13. The lint block, and what is not in it

The full policy is in the root `Cargo.toml`. Adopting it took the workspace from
0 to ~500 source-level warnings and back to 0, across `sbql-core` (105 in the
library plus 8 in its test modules), `sbql-tui` (~330, of which 246 were
`unreachable_pub` and 48 `let_underscore_drop`), and `sbql-ffi` (62, almost all
`use_self`).

Adopt anything further in stages: each line added must be clean before the next
lands, so `main` is never yellow.

Deliberately **not** adopted, with reasons:

| Lint | Why not |
| --- | --- |
| `pedantic` wholesale | ~500 warnings on top of what we took. Adopt the useful members individually. |
| `option_if_let_else` | Nursery, and its suggestion is regularly less readable than the original. |
| `redundant_pub_crate` | Cosmetic, and it fights `unreachable_pub`. Note that `pub` fields inside a `pub(crate)` struct are already capped and are not worth churning. |
| `must_use_candidate` | Would put `#[must_use]` on ~40 getters for little gain. Apply it by hand to builder methods that return `Self`. |
| `too_many_lines` | Real signal, but 31 hits means it is a refactoring backlog, not a lint. |
| `cast_possible_truncation` | The diagram canvas casts between screen and model coordinates constantly. Revisit together with `indexing_slicing`. |
| `missing_docs` outside `sbql-core` | `sbql-tui` and `sbql-ffi` are `publish = false`; their surface is internal. |

One thing that looks like a gap and is not: `sbql-ffi` needs no `unsafe_code`
exemption. uniffi 0.29 genuinely emits `unsafe`, but rustc suppresses lints in
external-macro spans, so none of it reaches the lint — while a hand-written
`unsafe` in that crate still warns. Verified by probe, not by absence. The lint
covers exactly the code we wrote, which is what we wanted.

---

## 14. Known gaps

Tracked here rather than in comments scattered across the tree.

1. ~~**`russh` 0.48 is vulnerable.**~~ **Fixed** by the bump to 0.62, which also
   dropped `russh-keys` (folded into `russh::keys` upstream) and `async-trait`.
   Two things from that port are worth keeping in mind rather than forgetting:
   `authenticate_publickey` now takes a `PrivateKeyWithHashAlg`, and passing
   `None` for the hash means ssh-rsa/SHA-1, which OpenSSH 8.8+ rejects by
   default — it surfaces to the user as "wrong key". And `check_server_key` is
   verified by a live `#[ignore]`d test in `tunnel.rs`; see §11.
2. ~~**SSH credentials are mismanaged in three related ways.**~~ **Fixed.** The
   write moved into `handlers::connection::save` (carried there by a new
   `ssh_password` field on `CoreCommand::SaveConnection`), so it lands beside
   the database password on the far side of `validate()` and gets the same
   `Severity::Warning` when the store refuses it. `delete_ssh_password` now
   exists and `handlers::connection::delete` calls it, and
   `save_ssh_password("")` deletes rather than returning `Ok(())` — see the doc
   comment there for why SSH does *not* follow `save_password`'s
   backend-specific empty-string rules. What remains is a product gap, not a
   correctness one: neither `sbql-tui` nor `ConnectionDraft` has an SSH
   password field, so only the macOS app can set one.
3. **The FFI has no channel for warnings.** `first_failure` correctly skips
   them, but nothing else carries them, so `extract_connection_list` drops them.
   The macOS app is therefore silent about keyring failures that `sbql-core`
   builds a careful `Severity::Warning` for. Fixing it changes the Swift-facing
   API.
4. **`tracing` from the FFI goes nowhere** — no subscriber on the macOS path.
   See §8.
5. **`Core::password_cache` is not cleared on delete.** Harmless for v4 UUIDs,
   but discovered Docker connections derive v5 ids from the container id, so
   deleting one and re-scanning silently reuses the cached password.
6. `indexing_slicing` and `significant_drop_tightening`, per §1 and §9.

---

## 15. Before you open a PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --lib --bins --tests
cargo check --workspace --all-targets          # benches and examples
make audit                                     # cargo-deny
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
  `unsafe_code`, `use_self`, `missing_const_for_fn`
- [`helix-editor/helix`](https://github.com/helix-editor/helix/blob/master/Cargo.toml) —
  release profile for a shipped TUI binary
- [Clippy lint index](https://doc.rust-lang.org/stable/clippy/lints.html),
  [rustdoc lints](https://doc.rust-lang.org/rustdoc/lints.html), and
  [RustSec](https://rustsec.org/)
