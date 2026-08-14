#!/bin/sh
# Install sbql, the terminal SQL workspace.
#
#   curl -fsSL https://raw.githubusercontent.com/Osmait/sbql/main/install.sh | sh
#
# Downloads the release binary for this machine, checks it against the
# published SHA-256, and puts it in ~/.local/bin. Nothing is built, nothing
# needs root, and nothing outside the install directory is touched.
#
# Knobs, all optional:
#   SBQL_VERSION       tag to install, e.g. v0.2.0 (default: the latest release)
#   SBQL_INSTALL_DIR   where to put the binary (default: ~/.local/bin)
#
# Plain POSIX sh on purpose: macOS still ships bash 3.2, and this has to run
# before the user has installed anything at all.

set -eu

REPO="Osmait/sbql"
# release-plz tags each crate separately, so the TUI's releases are the ones
# prefixed like this. The repository's "latest release" may well be a
# sbql-core tag with no binaries attached, which is why this script resolves
# the version itself instead of using /releases/latest/download.
TAG_PREFIX="sbql-tui-v"
BIN="sbql"

INSTALL_DIR="${SBQL_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "this needs \`$1\`, which is not installed"
}

# --- what are we running on ------------------------------------------------

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os_part="unknown-linux-gnu" ;;
        Darwin) os_part="apple-darwin" ;;
        *) die "no prebuilt binary for $os — build from source: https://github.com/$REPO#build-the-tui" ;;
    esac

    case "$arch" in
        x86_64 | amd64)  arch_part="x86_64" ;;
        arm64 | aarch64) arch_part="aarch64" ;;
        *) die "no prebuilt binary for $arch — build from source: https://github.com/$REPO#build-the-tui" ;;
    esac

    printf '%s-%s' "$arch_part" "$os_part"
}

# --- which release ---------------------------------------------------------

# The newest tag starting with TAG_PREFIX.
#
# Parsed with grep rather than jq so the one-liner works on a machine with
# nothing installed. The API lists releases newest first, so the first match
# is the one we want.
latest_version() {
    api="https://api.github.com/repos/$REPO/releases"
    curl -fsSL "$api" \
        | grep -o "\"tag_name\": *\"$TAG_PREFIX[^\"]*\"" \
        | head -n 1 \
        | sed 's/.*"\(.*\)"/\1/' \
        | sed "s/^$TAG_PREFIX/v/"
}

# --- go --------------------------------------------------------------------

main() {
    need curl
    need tar

    target="$(detect_target)"

    if [ -n "${SBQL_VERSION:-}" ]; then
        version="$SBQL_VERSION"
    else
        version="$(latest_version || true)"
        [ -n "$version" ] || die "could not find a published $BIN release — see https://github.com/$REPO/releases"
    fi
    # Accept both "0.2.0" and "v0.2.0".
    case "$version" in v*) ;; *) version="v$version" ;; esac

    tag="$TAG_PREFIX${version#v}"
    archive="$BIN-$target.tar.gz"
    base="https://github.com/$REPO/releases/download/$tag"

    say "Installing $BIN $version ($target)"

    tmp="$(mktemp -d)"
    # Leave nothing behind, including on a failed download or a Ctrl-C.
    trap 'rm -rf "$tmp"' EXIT INT TERM

    curl -fsSL "$base/$archive" -o "$tmp/$archive" \
        || die "no build for $target in $tag — see https://github.com/$REPO/releases/tag/$tag"
    curl -fsSL "$base/$archive.sha256" -o "$tmp/$archive.sha256" \
        || die "release $tag has no checksum for $archive; refusing to install unverified"

    verify "$tmp" "$archive"

    tar -xzf "$tmp/$archive" -C "$tmp"
    [ -f "$tmp/$BIN" ] || die "$archive did not contain $BIN"

    mkdir -p "$INSTALL_DIR"
    # Written to a temp name in the same directory and moved into place, so a
    # half-written binary can never end up on PATH — and so replacing a copy
    # that is currently running works instead of failing with "text file busy".
    chmod +x "$tmp/$BIN"
    mv "$tmp/$BIN" "$INSTALL_DIR/$BIN.new"
    mv "$INSTALL_DIR/$BIN.new" "$INSTALL_DIR/$BIN"

    say "Installed to $INSTALL_DIR/$BIN"
    check_path
    say ""
    say "Run \`$BIN\` to start. If you have databases running in Docker, it will offer them."
}

# Checksums are published as `<sha>  <name>`, the format both tools read.
verify() {
    dir="$1"
    name="$2"

    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$dir" && sha256sum -c "$name.sha256" >/dev/null 2>&1) \
            || die "checksum mismatch for $name — refusing to install"
    elif command -v shasum >/dev/null 2>&1; then
        (cd "$dir" && shasum -a 256 -c "$name.sha256" >/dev/null 2>&1) \
            || die "checksum mismatch for $name — refusing to install"
    else
        die "neither sha256sum nor shasum found; refusing to install unverified"
    fi
}

# Say so rather than editing the user's shell config behind their back.
check_path() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) return 0 ;;
    esac

    say ""
    say "$INSTALL_DIR is not on your PATH. Add it with one of:"
    say ""
    say "  echo 'export PATH=\"\$PATH:$INSTALL_DIR\"' >> ~/.bashrc"
    say "  echo 'export PATH=\"\$PATH:$INSTALL_DIR\"' >> ~/.zshrc"
    say "  fish_add_path $INSTALL_DIR"
}

main "$@"
