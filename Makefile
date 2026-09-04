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

.PHONY: vscode-deps
vscode-deps: ## Install VS Code extension dev dependencies
	cd vscode && npm ci

.PHONY: test-vscode
test-vscode: ## Run VS Code extension tests (needs cargo build and vscode-deps)
	cargo build
	cd vscode && npm run lint && npm test

.PHONY: vsix
vsix: ## Package the VS Code extension for TARGET=<rust triple> (binary from target/<triple>/release, or BIN=...)
	cd vscode && npm run compile && node out/scripts/vsix.js $(TARGET) $(BIN)

.PHONY: hooks
hooks: ## Install the repo's git hooks
	ln -sfn ../../.githooks/pre-commit "$$(git rev-parse --git-common-dir)/hooks/pre-commit"

.PHONY: fmt
fmt: ## Format Rust sources
	cargo fmt --all

.PHONY: bump
bump: ## Set the release version everywhere: make bump VERSION=x.y.z
	@case "$(VERSION)" in \
	  *[!0-9.]*|"") echo "usage: make bump VERSION=x.y.z" >&2; exit 1;; \
	  [0-9]*.[0-9]*.[0-9]*) ;; \
	  *) echo "usage: make bump VERSION=x.y.z" >&2; exit 1;; \
	esac
	sed -i '/^\[package\]/,/^\[/ s/^version = ".*"/version = "$(VERSION)"/' engine/Cargo.toml
	sed -i 's/^return ".*"/return "$(VERSION)"/' nvim/lua/whence/version.lua
	sed -i 's/^  "version": .*/  "version": "$(VERSION)",/' vscode/package.json
	sed -i -e 's/^  "version": .*/  "version": "$(VERSION)",/' \
	       -e '/^    "": {/,/^    },/ s/^      "version": .*/      "version": "$(VERSION)",/' \
	       vscode/package-lock.json
	cargo update --workspace --offline
	@$(MAKE) --no-print-directory release-check

.PHONY: release-check
release-check: ## Check engine, plugin, extension and tag ($$GITHUB_REF_NAME) versions agree
	@engine=$$(sed -n '/^\[package\]/,/^\[[^p]/ s/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1); \
	plugin=$$(sed -n 's/^return "\(.*\)"/\1/p' nvim/lua/whence/version.lua | head -1); \
	code=$$(sed -n 's/^  "version": "\(.*\)",/\1/p' vscode/package.json | head -1); \
	if [ -z "$$engine" ] || [ "$$engine" != "$$plugin" ]; then \
	  echo "version mismatch: engine/Cargo.toml '$$engine' vs nvim/lua/whence/version.lua '$$plugin'" >&2; exit 1; fi; \
	if [ "$$engine" != "$$code" ]; then \
	  echo "version mismatch: engine/Cargo.toml '$$engine' vs vscode/package.json '$$code'" >&2; exit 1; fi; \
	if [ -n "$$GITHUB_REF_NAME" ] && [ "$$GITHUB_REF_NAME" != "v$$engine" ]; then \
	  echo "tag $$GITHUB_REF_NAME does not match version v$$engine" >&2; exit 1; fi; \
	echo "version $$engine"
