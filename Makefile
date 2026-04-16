# wtch Makefile
# Run from WSL. Windows commands are proxied via powershell.exe.

SHELL := /bin/bash
PS := powershell.exe -NoProfile -Command
WIN_DIR := P:/dev/personal/wtch

# Development

.PHONY: kill
kill:
	-$(PS) "taskkill /F /IM wtch.exe 2>'$$null'; taskkill /F /IM node.exe 2>'$$null'"
	@sleep 2

.PHONY: dev
dev: kill ## Start Tauri dev mode (Windows, hot reload)
	$(PS) "cd '$(WIN_DIR)'; ./devops/run/dev.ps1"

.PHONY: test
test: ## Run Rust tests in Docker
	docker compose run --rm dev bash -c "cd src-tauri && cargo test"

.PHONY: check
check: ## Run cargo check in Docker
	docker compose run --rm dev bash -c "cd src-tauri && cargo check"

.PHONY: build-frontend
build-frontend: ## Build Vue frontend in Docker
	docker compose run --rm dev bash -c "npm install && npm run build"

# Build and Release (Windows)

.PHONY: build
build: ## Build .exe via PowerShell
	$(PS) "cd '$(WIN_DIR)'; ./devops/build/build.ps1"

.PHONY: release
release: ## Create GitHub release (usage: make release VERSION=0.1.0)
	$(PS) "cd '$(WIN_DIR)'; ./devops/build/release.ps1 -Version $(VERSION)"

# Docker

.PHONY: docker-build
docker-build: ## Build Docker dev image
	docker compose build dev

.PHONY: docker-shell
docker-shell: ## Open a shell in the Docker dev container
	docker compose run --rm dev bash

.PHONY: docker-down
docker-down: ## Stop and remove Docker containers/volumes
	docker compose down -v

# Install (Windows)

.PHONY: install-win
install-win: ## Install npm deps on Windows (with postinstall workaround)
	$(PS) "cd '$(WIN_DIR)'; npm install --ignore-scripts; node node_modules/esbuild/install.js"

# Cleanup

.PHONY: clean
clean: ## Remove build artifacts
	rm -rf dist/

.PHONY: clean-node
clean-node: ## Remove node_modules (Windows side)
	$(PS) "cd '$(WIN_DIR)'; cmd /c 'rmdir /s /q node_modules'"

# Help

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help
