# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/Osmait/sbql/compare/sbql-tui-v0.3.0...sbql-tui-v0.3.1) - 2026-08-14

### Added

- *(tui)* nine themes and a picker that previews as you move
- *(tui)* Alt+hjkl navigates panels wherever Ctrl+hjkl does
- *(tui)* Ctrl+hjkl moves between panels from anywhere

### Fixed

- *(tui)* let scrolling up cross a page boundary too
- *(tui)* keep scrolling smooth across a page boundary

### Other

- fold the two branches that land at the top

## [0.2.0](https://github.com/Osmait/sbql/compare/sbql-tui-v0.1.0...sbql-tui-v0.2.0) - 2026-07-28

### Added

- Ctrl+E opens the full text of a status-bar message, with its cause and a
  next step chosen from the error kind. The bar is one row and cannot wrap, so
  long database errors used to be cut off unreadably
  ([#8](https://github.com/Osmait/sbql/pull/8))

### Fixed

- **Losing the terminal input stream no longer leaves an unquittable window.**
  In raw mode Ctrl+C is a keypress, not a signal, so the only way out was to
  kill the process from another terminal
  ([#8](https://github.com/Osmait/sbql/pull/8))
- A failed or half-finished terminal takeover is now rolled back, instead of
  leaving the shell in raw mode with no echo
  ([#8](https://github.com/Osmait/sbql/pull/8))
- `status_msg` and `error_msg` merged into one notice, so a stale error can no
  longer hide the message that came after it
  ([#8](https://github.com/Osmait/sbql/pull/8))
- The log moved off the shared, predictable `/tmp/sbql.log` to a per-user state
  directory (or `$SBQL_LOG`), and a logging failure no longer prevents startup
  ([#8](https://github.com/Osmait/sbql/pull/8))
- `RUST_LOG` is honoured; fixed `sbql_*=info` directives used to override it
  ([#8](https://github.com/Osmait/sbql/pull/8))

## [0.1.2](https://github.com/Osmait/sbql/compare/sbql-tui-v0.1.0...sbql-tui-v0.1.2) - 2026-03-07

### Added

- *(diagram)* comprehensive visual improvements and navigation ([#5](https://github.com/Osmait/sbql/pull/5))

### Fixed

- *(release)* use workspace-pinned local core dependency

### Other

- release v0.1.1 ([#4](https://github.com/Osmait/sbql/pull/4))

## [0.1.1](https://github.com/Osmait/sbql/compare/sbql-tui-v0.1.0...sbql-tui-v0.1.1) - 2026-03-07

### Fixed

- *(release)* use workspace-pinned local core dependency

## [0.1.0](https://github.com/Osmait/sbql/releases/tag/sbql-tui-v0.1.0) - 2026-03-06

### Other

- Add release automation and live column filter updates
- Add filter autocomplete pipeline and robust text-cast filtering
- Improve result decoding and adopt global/panel navigation flow
- Initial sbql MVP: core engine, TUI workflows, and diagram UX
