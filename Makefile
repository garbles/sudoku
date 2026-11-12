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
# The name of the change in a development build, for example DEV_NAME=update-icons. The Settings
# panel then shows the version dev/update-icons. Empty: the version dev.
DEV_NAME ?=
DEV_VERSION = dev$(if $(strip $(DEV_NAME)),/$(strip $(DEV_NAME)))
CORPUS_DIR := /home/root/.local/share/rm-sudoku-corpus
CORPUS_LOG := corpus-ink.jsonl
CORPUS_OUT ?= target/corpus
DIGIT ?=
NOTE ?=
SCENARIO ?= first

# Installs the AppLoad folder dist/$(1) on the tablet and examines its checksum. $(2) is the name of
# the app in AppLoad.
define install_app
	@echo "Install on $(HOST)"
	COPYFILE_DISABLE=1 tar -C dist -czf - $(1) | $(SSH) 'set -e; d=$(APPLOAD); a=$(1); t=$$d/.$$a-install; \
		rm -rf "$$t"; mkdir -p "$$t"; tar -xzf - -C "$$t"; chmod 755 "$$t/$$a/sudoku"; \
		rm -rf "$$d/$$a.old"; if [ -d "$$d/$$a" ]; then mv "$$d/$$a" "$$d/$$a.old"; fi; \
		mv "$$t/$$a" "$$d/$$a"; rm -rf "$$d/$$a.old" "$$t"'
	@local=$$(shasum -a 256 dist/$(1)/sudoku | cut -d' ' -f1); \
		remote=$$($(SSH) 'sha256sum $(APPLOAD)/$(1)/sudoku' | cut -d' ' -f1); \
		if [ "$$local" = "$$remote" ]; then echo "Installed: $$local. Close $(2) and open it again."; \
		else echo "The checksums are not equal: $$local $$remote" >&2; exit 1; fi
endef

# The stamp file of the usual ink patch. A shell command on the tablet for install_patch.
PATCH_STAMP_CMD = printf "{\"version\": \"$(DEV_VERSION)\", \"sha256\": \"%s\"}\n" '"$$want"' > "$$d/$(PATCH_STAMP)"

# Installs the ink patch dist/$(1)/$(2) on the tablet if it changed. $(3) is a shell command on the
# tablet after the copy ($$d is the folder of the patches, $$want is the checksum of the patch).
define install_patch
	@want=$$(shasum -a 256 dist/$(1)/$(2) | cut -d' ' -f1); \
		have=$$($(SSH) 'sha256sum $(PATCH_DIR)/$(2) 2>/dev/null' | cut -d' ' -f1); \
		if [ "$$want" = "$$have" ]; then echo "The ink patch $(2) is installed."; else \
		$(SSH) 'set -e; d=$(PATCH_DIR); mkdir -p "$$d"; cat > "$$d/$(2).new"; mv "$$d/$(2).new" "$$d/$(2)"; $(3)' \
			< dist/$(1)/$(2); \
		echo "The ink patch $(2) changed. xovi applies it when it starts: run make xovi-restart."; fi
endef

.DEFAULT_GOAL := help
.PHONY: help test build snapshot check invariants video video-full media bench bench-save bench-linux \
	bench-linux-save media-images media-installer media-video fonts-check test-linux tablet-build tablet-build-dev tablet-build-corpus tablet-build-colors dist install-dev install-sim install-corpus install-colors colors-pull corpus-start corpus-pull fonts xovi-restart fmt clippy bump-patch bump-minor \
	bump-major clean

help: ## Show all targets
	@grep -hE '^[a-z-]+:.*## ' $(firstword $(MAKEFILE_LIST)) | awk -F':.*## ' '{printf "  \033[1m%-18s\033[0m %s\n", $$1, $$2}'

## --- Computer -------------------------------------------------------------------------------

test: ## Run all tests on the computer
	cargo test

build: ## Build the release executables for the computer
	cargo build --release

snapshot: ## Draw the reference states and the icon sheet and compare them with images/pro/ (SNAPSHOT_DIR=...)
	cargo run --release --bin snapshot -- $(SNAPSHOT_DIR)

check: test snapshot ## The checks before a change: tests and snapshots

invariants: ## Show the rules of INVARIANTS.md and the rules that changed, in the browser (BASE=<git ref>)
	python3 scripts/invariants.py --open $(if $(BASE),--base $(BASE))

fmt: ## Format the source code
	cargo fmt

clippy: ## Run the Rust linter
	cargo clippy --all-targets

## --- Videos ---------------------------------------------------------------------------------

video: ## Make video/demo.mp4 and video/demo.gif (needs ffmpeg)
	cargo run --release --bin video -- video/demo.txt video/demo.mp4

video-full: ## Make video/demo-full.mp4 and video/demo-full-move.mp4 for the website (needs ffmpeg)
	cargo run --release --bin video -- --no-gif video/demo-full.txt video/demo-full.mp4
	cargo run --release --bin video -- --no-gif --device move video/demo-full.txt video/demo-full-move.mp4

# The media targets make the files with fixed tool versions (Docker). Commit the result.
media: media-images media-installer media-video ## Make images/, the icon and video/ (Docker; make -j3 media for parallel builds)

media-images: fonts-check ## Make the design images images/pro/, the icon and the icon sheet (Dockerfile.images)
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.images --output type=local,dest=. .

media-installer: ## Make the installer images images/install-*.png (Dockerfile.installer)
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.installer --output type=local,dest=. .

media-video: fonts-check ## Make video/demo.mp4, video/demo.gif, video/demo-full.mp4 and video/demo-full-move.mp4 (Dockerfile.video)
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.video --output type=local,dest=. .

## --- Performance ----------------------------------------------------------------------------

bench: ## Run the bench and compare it with bench/baseline-computer.json (NOTE=...)
	scripts/bench.sh $(if $(NOTE),--note "$(NOTE)")

bench-save: ## Run the bench and save a new computer baseline (NOTE=...)
	scripts/bench.sh --save $(if $(NOTE),--note "$(NOTE)")

BENCH_LINUX = $(DOCKER) run --rm --platform linux/arm64 -v "$(CURDIR)":/src -w /src \
	-e CARGO_TARGET_DIR=/tmp/target rust:1.97-alpine \
	sh -c 'apk add --no-cache musl-dev bash python3 git >/dev/null && scripts/bench.sh --baseline bench/baseline-linux.json $(1)'

bench-linux: ## Run the bench in the Linux container (musl) and compare it with bench/baseline-linux.json
	$(call BENCH_LINUX,$(if $(NOTE),--note "$(NOTE)"))

bench-linux-save: ## Run the bench in the Linux container and save a new Linux baseline
	$(call BENCH_LINUX,--save $(if $(NOTE),--note "$(NOTE)"))

## --- Tablet ---------------------------------------------------------------------------------

fonts-check:
	@test -f scripts/fonts/rmsans-400.ttf -a -f scripts/fonts/rmsans-500.ttf \
		|| { echo "The fonts are missing. Run make fonts (refer to scripts/fonts/README.md)." >&2; exit 1; }

test-linux: fonts-check ## Run all tests on aarch64 Linux, release and debug in parallel (Dockerfile.test)
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.test .

tablet-build: ## Build dist/sudoku/ with no tests (Dockerfile)
	$(DOCKER) build --platform linux/arm64 --output type=local,dest=dist .

# The limit of 14 characters keeps the version clear of the name "Version" in the Settings panel.
tablet-build-dev: ## Build dist/sudoku/ fast, for development only: no LTO, incremental, cache (Dockerfile.dev). DEV_NAME=update-icons sets the version dev/update-icons
	@[[ "$(DEV_NAME)" =~ ^([a-z0-9][a-z0-9-]{0,13})?$$ ]] \
		|| { echo "DEV_NAME must have 1 to 14 lowercase letters, digits or hyphens, and start with a letter or a digit: $(DEV_NAME)" >&2; exit 1; }
	@[ -n "$(strip $(DEV_NAME))" ] \
		|| echo "No DEV_NAME: the version is dev. Give a name that tells the change, for example DEV_NAME=update-icons." >&2
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.dev --build-arg SUDOKU_VERSION="$(DEV_VERSION)" --output type=local,dest=dist .

dist: test-linux tablet-build ## Run all tests on aarch64 Linux, then build dist/sudoku/ (Docker)

install-dev: tablet-build-dev ## Build fast (Dockerfile.dev, no tests: run make test first) and install dist/sudoku/ on the tablet (REMARKABLE_HOST in .env, or USB). Give DEV_NAME=<name of the change>
	$(call install_app,sudoku,Sudoku)
	$(call install_patch,sudoku,$(PATCH_NAME),$(PATCH_STAMP_CMD))

## The corpus build: examples of handwriting for the recognizer (the skill remarkable-corpus). It is
## the separate app Sudoku (corpus), with its own AppLoad folder and data folder.

tablet-build-corpus: ## Build dist/sudoku-corpus/ fast, for development only (Dockerfile.corpus)
	rm -rf dist/sudoku-corpus
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.corpus --output type=local,dest=dist .

install-corpus: tablet-build-corpus ## Build and install the separate app Sudoku (corpus): a blank board, a log of each ink. The usual Sudoku app does not change
	$(call install_app,sudoku-corpus,Sudoku (corpus))
	$(call install_patch,sudoku-corpus,sudoku-corpus-penregion.qmd,true)

## The color picker: select the colors of the palette on the display. It is the separate app
## Sudoku (colors), with its own AppLoad folder and data folder. It has no ink patch.

tablet-build-colors: ## Build dist/sudoku-colors/ fast, for development only (Dockerfile.colors)
	rm -rf dist/sudoku-colors
	$(DOCKER) build --platform linux/arm64 -f Dockerfile.colors --output type=local,dest=dist .

install-colors: tablet-build-colors ## Build and install the separate app Sudoku (colors): a color picker. The usual Sudoku app does not change
	$(call install_app,sudoku-colors,Sudoku (colors))

colors-pull: ## Copy the colors of Sudoku (colors) from the tablet to target/colors/colors.json (the skill remarkable-colors)
	@mkdir -p target/colors
	@$(SSH) 'cat /home/root/.local/share/rm-sudoku-colors/colors.json' > target/colors/colors.json.tmp \
		|| { rm -f target/colors/colors.json.tmp; echo "Cannot read colors.json on the tablet. Change a color in Sudoku (colors) first." >&2; exit 1; }
	@mv target/colors/colors.json.tmp target/colors/colors.json
	@echo "Wrote target/colors/colors.json"

corpus-start: ## Start a new corpus session: delete the corpus board and the corpus log on the tablet, move old sets to target/corpus/old/ (close Sudoku (corpus) first)
	$(SSH) 'rm -f $(CORPUS_DIR)/game.json $(CORPUS_DIR)/$(CORPUS_LOG)'
	@old=$(CORPUS_OUT)/old/$$(date +%Y%m%d-%H%M%S); for f in $(CORPUS_OUT)/corpus-ink-*.jsonl $(CORPUS_OUT)/sheet-*.png; do \
		[ -e "$$f" ] || continue; mkdir -p "$$old"; mv "$$f" "$$old/"; done; \
		if [ -d "$$old" ]; then echo "Old sets moved to $$old (not used)."; fi
	@echo "The corpus board and the corpus log are deleted. Open Sudoku (corpus) on the tablet."

corpus-pull: ## Copy the corpus log of DIGIT=n to target/corpus/corpus-ink-n.jsonl, then clear the log and the board on the tablet (close Sudoku (corpus) first)
	@test -n "$(DIGIT)" || { echo "Give the digit: make corpus-pull DIGIT=4" >&2; exit 1; }
	@mkdir -p $(CORPUS_OUT)
	@out=$(CORPUS_OUT)/corpus-ink-$(DIGIT).jsonl; \
		if [ -s "$$out" ]; then echo "$$out exists: add it or move it first" >&2; exit 1; fi; \
		$(SSH) 'f=$(CORPUS_DIR)/$(CORPUS_LOG); [ ! -f "$$f" ] || cat "$$f"' > "$$out.tmp" \
			|| { rm -f "$$out.tmp"; echo "Cannot read the corpus log on the tablet" >&2; exit 1; }; \
		if [ ! -s "$$out.tmp" ]; then rm -f "$$out.tmp"; echo "The corpus log on the tablet is empty" >&2; exit 1; fi; \
		mv "$$out.tmp" "$$out"; \
		$(SSH) 'rm -f $(CORPUS_DIR)/$(CORPUS_LOG) $(CORPUS_DIR)/game.json'; \
		echo "$$(grep -c '"stroke"' "$$out") strokes of $(DIGIT) in $$out. The log on the tablet is clear."

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

# Increase the version in Cargo.toml and Cargo.lock, make the commit <version> and the tag
# v<version>. cargo set-version changes only the two files, thus this recipe runs the git commands.
# The git working tree must be clean, thus the commit contains only the version.
define bump_version
	@if [ -n "$$(git status --porcelain)" ]; then \
		echo "The git working tree is not clean. Commit or remove these changes first:" >&2; \
		git status --short >&2; \
		exit 1; \
	fi
	cargo set-version --bump $(1)
	@version="$$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])')"; \
	if git rev-parse -q --verify "refs/tags/v$$version" > /dev/null; then \
		echo "The tag v$$version is already on this computer. The version is not committed." >&2; \
		echo "Delete the tag, or restore the files: git restore Cargo.toml Cargo.lock" >&2; \
		exit 1; \
	fi; \
	git commit -m "$$version" -- Cargo.toml Cargo.lock; \
	git tag "v$$version"; \
	echo "Made the commit $$version and the tag v$$version."; \
	echo "Push them: git push origin main v$$version"
endef

bump-patch: ## Increase the patch version: commit it and tag v<version>
	$(call bump_version,patch)

bump-minor: ## Increase the minor version: commit it and tag v<version>
	$(call bump_version,minor)

bump-major: ## Increase the major version: commit it and tag v<version>
	$(call bump_version,major)

clean: ## Remove target/ and dist/
	cargo clean
	rm -rf dist
