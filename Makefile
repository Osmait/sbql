.PHONY: help release-plz-install version-auto version-auto-dry release-dry-run release install-local reinstall-local uninstall-local build-release

help:
	@printf "\nSBQL Make targets:\n\n"
	@printf "  make release-plz-install  Install release-plz CLI\n"
	@printf "  make version-auto         Auto-bump workspace versions/changelog (release-plz update)\n"
	@printf "  make version-auto-dry     Show version changes without writing files\n"
	@printf "  make release-dry-run      Simulate release/tag flow\n"
	@printf "  make release              Run release-plz release\n"
	@printf "  make build-release        Build sbql release binary\n"
	@printf "  make install-local        Install sbql locally (~/.cargo/bin/sbql)\n"
	@printf "  make reinstall-local      Force reinstall local sbql binary\n"
	@printf "  make uninstall-local      Remove local sbql binary\n\n"

release-plz-install:
	cargo install release-plz --locked

version-auto:
	@command -v release-plz >/dev/null || (echo "release-plz not found. Run: make release-plz-install" && exit 1)
	release-plz update

version-auto-dry:
	@command -v release-plz >/dev/null || (echo "release-plz not found. Run: make release-plz-install" && exit 1)
	release-plz update --dry-run

release-dry-run:
	@command -v release-plz >/dev/null || (echo "release-plz not found. Run: make release-plz-install" && exit 1)
	release-plz release --dry-run

release:
	@command -v release-plz >/dev/null || (echo "release-plz not found. Run: make release-plz-install" && exit 1)
	release-plz release

build-release:
	cargo build --release

install-local:
	cargo install --path sbql-tui --locked

reinstall-local:
	cargo install --path sbql-tui --locked --force

uninstall-local:
	cargo uninstall sbql || true
