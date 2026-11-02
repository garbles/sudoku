# The usual operations of this repository. AGENTS.md gives the details of each command.
#
#   make help        shows all targets
#
# Docker: if `docker` is not on PATH, the Makefile uses the docker of OrbStack.

SHELL := /bin/bash

ifeq ($(shell command -v docker 2>/dev/null),)
DOCKER ?= DOCKER_HOST=unix://$(HOME)/.orbstack/run/docker.sock /Applications/OrbStack.app/Contents/MacOS/xbin/docker
else
DOCKER ?= docker
endif

# The address of the tablet: REMARKABLE_HOST in .env. Empty: the USB connection (10.11.99.1).
-include .env
HOST := $(if $(strip $(REMARKABLE_HOST)),$(REMARKABLE_HOST),10.11.99.1)
SSH_KEY ?= $(or $(SUDOKU_SSH_KEY),$(HOME)/.ssh/id_ed25519_remarkable)
SSH := ssh -o ConnectTimeout=5 -o BatchMode=yes -o IdentitiesOnly=yes \
	-o StrictHostKeyChecking=accept-new -o LogLevel=ERROR -i "$(SSH_KEY)" root@$(HOST)
APPLOAD := /home/root/xovi/exthome/appload
PATCH_DIR := /home/root/xovi/exthome/qt-resource-rebuilder
PATCH_NAME := sudoku-penregion.qmd
PATCH_STAMP := sudoku-penregion.json

SNAPSHOT_DIR ?= target/snapshots
NOTE ?=
SCENARIO ?= first

.DEFAULT_GOAL := help
.PHONY: help test build snapshot check video video-full media bench bench-save bench-linux \
	bench-linux-save imagegen dist install-dev install-sim fonts xovi-restart fmt clippy bump-patch bump-minor \
	bump-major clean

help: ## Show all targets
	@grep -hE '^[a-z-]+:.*## ' $(firstword $(MAKEFILE_LIST)) | awk -F':.*## ' '{printf "  \033[1m%-18s\033[0m %s\n", $$1, $$2}'

## --- Computer -------------------------------------------------------------------------------

test: ## Run all tests on the computer
	cargo test

build: ## Build the release executables for the computer
	cargo build --release

snapshot: ## Draw the reference states and compare them with images/ (SNAPSHOT_DIR=...)
	cargo run --release --bin snapshot -- $(SNAPSHOT_DIR)

check: test snapshot ## The checks before a change: tests and snapshots

fmt: ## Format the source code
	cargo fmt

clippy: ## Run the Rust linter
	cargo clippy --all-targets

## --- Videos ---------------------------------------------------------------------------------

video: ## Make video/demo.mp4 and video/demo.gif (needs ffmpeg)
	cargo run --release --bin video -- video/demo.txt video/demo.mp4

video-full: ## Make video/demo-full.mp4 for the website (needs ffmpeg)
	cargo run --release --bin video -- --no-gif video/demo-full.txt video/demo-full.mp4

media: imagegen ## Alias of imagegen

imagegen: ## Make images/, the icon and video/ with fixed tool versions (Docker). Commit the result.
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.imagegen --output type=local,dest=. .

## --- Performance ----------------------------------------------------------------------------

bench: ## Run the bench and compare it with bench/baseline-computer.json (NOTE=...)
	scripts/bench.sh $(if $(NOTE),--note "$(NOTE)")

bench-save: ## Run the bench and save a new computer baseline (NOTE=...)
	scripts/bench.sh --save $(if $(NOTE),--note "$(NOTE)")

BENCH_LINUX = $(DOCKER) run --rm --platform linux/arm64 -v "$(CURDIR)":/src -w /src \
	-e CARGO_TARGET_DIR=/tmp/target rust:1.97-alpine \
	sh -c 'apk add --no-cache musl-dev bash python3 >/dev/null && scripts/bench.sh --baseline bench/baseline-linux.json $(1)'

bench-linux: ## Run the bench in the Linux container (musl) and compare it with bench/baseline-linux.json
	$(call BENCH_LINUX,$(if $(NOTE),--note "$(NOTE)"))

bench-linux-save: ## Run the bench in the Linux container and save a new Linux baseline
	$(call BENCH_LINUX,--save $(if $(NOTE),--note "$(NOTE)"))

## --- Tablet ---------------------------------------------------------------------------------

dist: ## Run all tests on aarch64 Linux and build dist/sudoku/ (Docker)
	@test -f scripts/fonts/rmsans-400.ttf -a -f scripts/fonts/rmsans-500.ttf \
		|| { echo "The fonts are missing. Run make fonts (refer to scripts/fonts/README.md)." >&2; exit 1; }
	$(DOCKER) build --platform linux/arm64 --output type=local,dest=dist .

install-dev: dist ## Build and install dist/sudoku/ on the tablet (REMARKABLE_HOST in .env, or USB)
	@echo "Install on $(HOST)"
	COPYFILE_DISABLE=1 tar -C dist -czf - sudoku | $(SSH) 'set -e; d=$(APPLOAD); t=$$d/.sudoku-install; \
		rm -rf "$$t"; mkdir -p "$$t"; tar -xzf - -C "$$t"; chmod 755 "$$t/sudoku/sudoku"; \
		rm -rf "$$d/sudoku.old"; if [ -d "$$d/sudoku" ]; then mv "$$d/sudoku" "$$d/sudoku.old"; fi; \
		mv "$$t/sudoku" "$$d/sudoku"; rm -rf "$$d/sudoku.old" "$$t"'
	@local=$$(shasum -a 256 dist/sudoku/sudoku | cut -d' ' -f1); \
		remote=$$($(SSH) 'sha256sum $(APPLOAD)/sudoku/sudoku' | cut -d' ' -f1); \
		if [ "$$local" = "$$remote" ]; then echo "Installed: $$local. Close Sudoku and open it again."; \
		else echo "The checksums are not equal: $$local $$remote" >&2; exit 1; fi
	@want=$$(shasum -a 256 dist/sudoku/$(PATCH_NAME) | cut -d' ' -f1); \
		have=$$($(SSH) 'sha256sum $(PATCH_DIR)/$(PATCH_NAME) 2>/dev/null' | cut -d' ' -f1); \
		if [ "$$want" = "$$have" ]; then echo "The ink patch is installed."; else \
		$(SSH) 'set -e; d=$(PATCH_DIR); mkdir -p "$$d"; cat > "$$d/$(PATCH_NAME).new"; mv "$$d/$(PATCH_NAME).new" "$$d/$(PATCH_NAME)"; \
			printf "{\"version\": \"dev\", \"sha256\": \"%s\"}\n" '"$$want"' > "$$d/$(PATCH_STAMP)"' \
			< dist/sudoku/$(PATCH_NAME); \
		echo "The ink patch changed. xovi applies it when it starts: run make xovi-restart."; fi

xovi-restart: ## Start xovi again on the tablet (the UI starts again: close all notebooks first)
	$(SSH) '/home/root/xovi/start > /tmp/sudoku-xovi.log 2>&1; i=0; while [ $$i -lt 30 ]; do \
		p=$$(pidof xochitl) && grep -q xovi.so /proc/$${p%% *}/maps 2>/dev/null && exit 0; sleep 1; i=$$((i + 1)); done; \
		tail -n 20 /tmp/sudoku-xovi.log; exit 1'
	@echo "xovi is on."

install-sim: ## Run the installer in a simulation (SCENARIO=first|connect|again|...)
	bash tests/install/run.sh $(SCENARIO)

FONTS_VENV := target/fonts-venv
FONTS_SRC := /usr/share/remarkable/webui/reMarkableSans.woff2

fonts: ## Copy reMarkable Sans from the tablet and make scripts/fonts/rmsans-{400,500}.ttf
	@echo "Copy the font from $(HOST)"
	$(SSH) 'cat $(FONTS_SRC)' > target/reMarkableSans.woff2
	test -x $(FONTS_VENV)/bin/python || python3 -m venv $(FONTS_VENV)
	$(FONTS_VENV)/bin/pip install --quiet fonttools brotli
	$(FONTS_VENV)/bin/python -c 'from fontTools.ttLib import TTFont; \
		from fontTools.varLib.instancer import instantiateVariableFont; \
		exec("for w in (400, 500):\n f = TTFont(\"target/reMarkableSans.woff2\"); f.flavor = None\n instantiateVariableFont(f, {\"wght\": w, \"ital\": 0}, inplace=True)\n f.save(f\"scripts/fonts/rmsans-{w}.ttf\")")'
	rm -f target/reMarkableSans.woff2
	@ls -l scripts/fonts/rmsans-*.ttf

## --- Release --------------------------------------------------------------------------------

bump-patch: ## Increase the patch version (Cargo.toml and Cargo.lock)
	cargo set-version --bump patch

bump-minor: ## Increase the minor version (Cargo.toml and Cargo.lock)
	cargo set-version --bump minor

bump-major: ## Increase the major version (Cargo.toml and Cargo.lock)
	cargo set-version --bump major

clean: ## Remove target/ and dist/
	cargo clean
	rm -rf dist
