.PHONY: help release-plz-install version-auto version-auto-dry release-dry-run release install-local reinstall-local uninstall-local build-release xcframework install-macos uninstall-macos bench bench-pg bench-redis bench-all bench-report profile-memory flamegraph flamegraph-install

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
	@printf "  make uninstall-local      Remove local sbql binary\n"
	@printf "  make xcframework          Build XCFramework + Swift bindings\n"
	@printf "  make install-macos        Build and install macOS app to /Applications\n"
	@printf "  make uninstall-macos      Remove macOS app from /Applications\n"
	@printf "\nPerformance:\n\n"
	@printf "  make bench                Run Criterion benchmarks (no Docker)\n"
	@printf "  make bench-pg             Run PostgreSQL integration benchmarks (Docker)\n"
	@printf "  make bench-redis          Run Redis integration benchmarks (Docker)\n"
	@printf "  make bench-all            Run all benchmarks including Docker\n"
	@printf "  make bench-report         Open Criterion HTML reports\n"
	@printf "  make profile-memory       Run dhat heap profiling workload\n"
	@printf "  make flamegraph           Generate CPU flamegraph from benchmarks\n"
	@printf "  make flamegraph-install   Install cargo-flamegraph\n\n"

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

xcframework:
	./scripts/build-xcframework.sh

install-macos: xcframework
	@echo "==> Building macOS app (Release)..."
	xcodebuild -project sbql-macos/sbql-macos.xcodeproj \
		-scheme sbql-macos \
		-configuration Release \
		-derivedDataPath build/DerivedData \
		-quiet
	@echo "==> Installing to /Applications..."
	@rm -rf /Applications/sbql-macos.app
	cp -R build/DerivedData/Build/Products/Release/sbql-macos.app /Applications/
	@echo "==> Done! sbql-macos.app installed to /Applications/"

uninstall-macos:
	rm -rf /Applications/sbql-macos.app
	@echo "==> sbql-macos.app removed from /Applications/"

# ---------------------------------------------------------------------------
# Performance
# ---------------------------------------------------------------------------

bench:
	cargo bench --package sbql-core --bench query_builder --bench query_execution

bench-pg:
	cargo bench --package sbql-core --bench postgres_integration

bench-redis:
	cargo bench --package sbql-core --bench redis_integration

bench-all:
	cargo bench --package sbql-core

bench-report:
	@if [ -f target/criterion/report/index.html ]; then \
		open target/criterion/report/index.html; \
	else \
		echo "No report found. Run 'make bench' first."; \
		exit 1; \
	fi

profile-memory:
	cargo run --package sbql-core --example profile_memory --features dhat-heap

flamegraph:
	@command -v cargo-flamegraph >/dev/null || (echo "flamegraph not found. Run: make flamegraph-install" && exit 1)
	cargo flamegraph --package sbql-core --bench query_execution -- --bench

flamegraph-install:
	cargo install flamegraph
