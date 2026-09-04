BINARY     := orbit
INSTALL_DIR ?= $(HOME)/.local/bin
TARGET_DIR  := target/release
DEBUG_DIR   := target/debug
DEV_LINK    := dev-orbit
CANARY_LINK := orbit-canary

.PHONY: build build-release install uninstall dev-install dev-uninstall canary-install canary-uninstall package test clean help

## Build debug binary (fast, for development)
build:
	cargo build

## Build optimised release binary
build-release:
	cargo build --release

## Install the stable release binary to INSTALL_DIR (default: ~/.local/bin)
install: build-release
	@mkdir -p $(INSTALL_DIR)
	install -m 755 $(TARGET_DIR)/$(BINARY) $(INSTALL_DIR)/$(BINARY)
	@echo "Installed stable $(INSTALL_DIR)/$(BINARY)"
	@echo "For the dev build alongside it, run: make dev-install"

## Remove the stable binary (leaves the dev symlink; use dev-uninstall for that)
uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	@echo "Removed $(INSTALL_DIR)/$(BINARY)"

## Symlink dev-orbit -> local debug build (isolated home ~/.orbit-dev).
## Rebuild with `cargo build` and the symlink serves the new binary — no reinstall.
dev-install: build
	@mkdir -p $(INSTALL_DIR)
	ln -sf $(CURDIR)/$(DEBUG_DIR)/orbit-dev $(INSTALL_DIR)/$(DEV_LINK)
	@echo "Linked $(INSTALL_DIR)/$(DEV_LINK) -> $(CURDIR)/$(DEBUG_DIR)/orbit-dev"
	@echo "Runs against ~/.orbit-dev with its own daemon. Start it with: $(DEV_LINK) daemon start"

## Remove the dev symlink
dev-uninstall:
	rm -f $(INSTALL_DIR)/$(DEV_LINK)
	@echo "Removed $(INSTALL_DIR)/$(DEV_LINK)"

## Symlink orbit-canary -> local debug build (isolated home ~/.orbit-canary).
## Tracks canary pre-releases; runs its own daemon beside stable and dev.
canary-install: build
	@mkdir -p $(INSTALL_DIR)
	ln -sf $(CURDIR)/$(DEBUG_DIR)/orbit-canary $(INSTALL_DIR)/$(CANARY_LINK)
	@echo "Linked $(INSTALL_DIR)/$(CANARY_LINK) -> $(CURDIR)/$(DEBUG_DIR)/orbit-canary"
	@echo "Runs against ~/.orbit-canary with its own daemon. Start it with: $(CANARY_LINK) daemon start"

## Remove the canary symlink
canary-uninstall:
	rm -f $(INSTALL_DIR)/$(CANARY_LINK)
	@echo "Removed $(INSTALL_DIR)/$(CANARY_LINK)"

## Stage the release binary as a canonical release artifact for this host:
##   orbit-<os>-<arch>  (bare binary, os: linux|macos, arch: x86_64|aarch64)
## plus its line in checksums.txt — the exact names the self-updater expects.
## CI builds all platforms and merges each host's checksums.txt.
package: build-release
	$(eval OS := $(shell uname -s | sed 's/Linux/linux/;s/Darwin/macos/'))
	$(eval ARCH := $(shell uname -m | sed 's/arm64/aarch64/'))
	$(eval ARTIFACT := orbit-$(OS)-$(ARCH))
	cp $(TARGET_DIR)/$(BINARY) $(ARTIFACT)
	sha256sum $(ARTIFACT) > checksums.txt
	@echo "Artifact: $(ARTIFACT)"
	@cat checksums.txt

## Run all tests
test:
	cargo test

## Remove build artifacts
clean:
	cargo clean

## Show this help
help:
	@grep -E '^## ' Makefile | sed 's/^## //'
	@echo ""
	@echo "Variables:"
	@echo "  INSTALL_DIR  install destination (default: $(INSTALL_DIR))"
