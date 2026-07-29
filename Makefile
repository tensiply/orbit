BINARY     := orbit
INSTALL_DIR ?= $(HOME)/.local/bin
TARGET_DIR  := target/release

.PHONY: build build-release install uninstall package test clean help

## Build debug binary (fast, for development)
build:
	cargo build

## Build optimised release binary
build-release:
	cargo build --release

## Install release binary to INSTALL_DIR (default: ~/.local/bin)
install: build-release
	@mkdir -p $(INSTALL_DIR)
	install -m 755 $(TARGET_DIR)/$(BINARY) $(INSTALL_DIR)/$(BINARY)
	install -m 755 $(TARGET_DIR)/orbit-dev $(INSTALL_DIR)/orbit-dev
	@echo "Installed to $(INSTALL_DIR)/$(BINARY)"
	@echo "Installed to $(INSTALL_DIR)/orbit-dev"

## Remove installed binaries
uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY) $(INSTALL_DIR)/orbit-dev
	@echo "Removed $(INSTALL_DIR)/$(BINARY) and $(INSTALL_DIR)/orbit-dev"

## Build release binary and create a distributable archive
## Output: orbit-<version>-<platform>.tar.gz
package: build-release
	$(eval VERSION := $(shell $(TARGET_DIR)/$(BINARY) --version | awk '{print $$2}'))
	$(eval PLATFORM := $(shell uname -s | tr '[:upper:]' '[:lower:]')-$(shell uname -m))
	$(eval ARCHIVE := orbit-$(VERSION)-$(PLATFORM).tar.gz)
	tar -czf $(ARCHIVE) -C $(TARGET_DIR) $(BINARY) orbit-dev
	@echo "Package: $(ARCHIVE)"
	@sha256sum $(ARCHIVE)

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
