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
test-nvim: ## Run Neovim plugin tests (plenary path: PLENARY_DIR=~/.local/share/nvim/lazy/plenary.nvim)
	PLENARY_DIR=$(PLENARY_DIR) nvim --headless -u nvim/tests/minimal_init.lua -c "PlenaryBustedDirectory nvim/tests { minimal_init = 'nvim/tests/minimal_init.lua' }"

.PHONY: nvim-link
nvim-link: build ## Symlink release binary into the plugin's bin/
	mkdir -p nvim/bin && ln -sf $(CURDIR)/target/release/whence nvim/bin/whence

.PHONY: fmt
fmt: ## Format Rust sources
	cargo fmt --all
