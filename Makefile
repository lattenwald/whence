.DEFAULT_GOAL := help

PLENARY_DIR ?= $(HOME)/.local/share/nvim/lazy/plenary.nvim

.PHONY: help
help: ## Show targets
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

.PHONY: build
build: ## Build release engine
	cargo build --release

.PHONY: test
test: ## Run engine tests
	cargo test --workspace

.PHONY: test-nvim
test-nvim: ## Run Neovim plugin tests (override plenary path with PLENARY_DIR=...)
	PLENARY_DIR=$(PLENARY_DIR) nvim --headless -u nvim/tests/minimal_init.lua -c "PlenaryBustedDirectory nvim/tests { minimal_init = 'nvim/tests/minimal_init.lua' }"

.PHONY: nvim-link
nvim-link: build ## Symlink release binary into the plugin's bin/
	mkdir -p nvim/bin && ln -sf $(CURDIR)/target/release/whence nvim/bin/whence

.PHONY: fmt
fmt: ## Format Rust sources
	cargo fmt --all

.PHONY: release-check
release-check: ## Check engine, plugin and tag ($$GITHUB_REF_NAME) versions agree
	@engine=$$(sed -n '/^\[package\]/,/^\[[^p]/ s/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1); \
	plugin=$$(sed -n 's/^return "\(.*\)"/\1/p' nvim/lua/whence/version.lua | head -1); \
	if [ -z "$$engine" ] || [ "$$engine" != "$$plugin" ]; then \
	  echo "version mismatch: engine/Cargo.toml '$$engine' vs nvim/lua/whence/version.lua '$$plugin'" >&2; exit 1; fi; \
	if [ -n "$$GITHUB_REF_NAME" ] && [ "$$GITHUB_REF_NAME" != "v$$engine" ]; then \
	  echo "tag $$GITHUB_REF_NAME does not match version v$$engine" >&2; exit 1; fi; \
	echo "version $$engine"
