# KG MCP Server Makefile
# Production-ready Knowledge Graph MCP Server

.PHONY: all build release test clean install uninstall setup docker bench lint check deps

# Configuration
BINARY_NAME := kg-mcp-server
SETUP_BINARY := kg-setup
MIGRATE_BINARY := kg-migrate
INSTALL_DIR := $(HOME)/.local/bin
CURSOR_CONFIG_DIR := $(HOME)/.cursor
TARGET_DIR := target/release

# Default target
all: build

# Development build
build:
	@echo "🔨 Building KG MCP Server..."
	cargo build

# Production release build
release:
	@echo "🚀 Building production release..."
	cargo build --release
	@echo "✅ Release build complete: $(TARGET_DIR)/$(BINARY_NAME)"

# Run comprehensive tests
test:
	@echo "🧪 Running test suite..."
	cargo test --verbose
	@echo "✅ All tests passed!"

# Run benchmarks
bench:
	@echo "📊 Running performance benchmarks..."
	cargo bench
	@echo "✅ Benchmarks complete!"

# Lint and format code
lint:
	@echo "🔍 Linting code..."
	cargo clippy -- -D warnings
	cargo fmt --check
	@echo "✅ Code quality checks passed!"

# Check code without building
check:
	@echo "🔍 Checking code..."
	cargo check
	@echo "✅ Code check complete!"

# Install dependencies and setup development environment
deps:
	@echo "📦 Installing dependencies..."
	@if ! command -v cargo > /dev/null; then \
		echo "🦀 Installing Rust..."; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; \
		source ~/.cargo/env; \
	fi
	@echo "✅ Dependencies ready!"

# Install binaries to local bin directory
install: release
	@echo "📦 Installing KG MCP Server..."
	@mkdir -p $(INSTALL_DIR)
	@cp $(TARGET_DIR)/$(BINARY_NAME) $(INSTALL_DIR)/
	@cp $(TARGET_DIR)/$(SETUP_BINARY) $(INSTALL_DIR)/
	@cp $(TARGET_DIR)/$(MIGRATE_BINARY) $(INSTALL_DIR)/
	@chmod +x $(INSTALL_DIR)/$(BINARY_NAME)
	@chmod +x $(INSTALL_DIR)/$(SETUP_BINARY)
	@chmod +x $(INSTALL_DIR)/$(MIGRATE_BINARY)
	@echo "✅ Binaries installed to $(INSTALL_DIR)"
	@echo "🔧 Setting up Cursor integration..."
	@$(INSTALL_DIR)/$(SETUP_BINARY) cursor --global || echo "⚠️  Manual Cursor setup required"
	@echo "🎉 Installation complete!"

# Uninstall binaries
uninstall:
	@echo "🗑️  Uninstalling KG MCP Server..."
	@rm -f $(INSTALL_DIR)/$(BINARY_NAME)
	@rm -f $(INSTALL_DIR)/$(SETUP_BINARY)
	@rm -f $(INSTALL_DIR)/$(MIGRATE_BINARY)
	@echo "✅ Uninstallation complete!"

# Quick setup for development
setup: deps build
	@echo "🎯 Setting up development environment..."
	@mkdir -p $(CURSOR_CONFIG_DIR)
	@echo '{"mcpServers":{"kg-mcp-server":{"url":"http://localhost:8360/sse"}}}' > $(CURSOR_CONFIG_DIR)/mcp.json
	@echo "✅ Development setup complete!"
	@echo "💡 Run 'make start' to start the server"

# Start the server in development mode
start: build
	@echo "🚀 Starting KG MCP Server..."
	RUST_LOG=info cargo run --release

# Start the server in debug mode
debug:
	@echo "🐛 Starting KG MCP Server in debug mode..."
	RUST_LOG=debug cargo run

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	@echo "✅ Clean complete!"

# Generate Docker configuration
docker:
	@echo "🐳 Generating Docker configuration..."
	@if [ -x "$(INSTALL_DIR)/$(SETUP_BINARY)" ]; then \
		$(INSTALL_DIR)/$(SETUP_BINARY) docker; \
	elif [ -x "$(TARGET_DIR)/$(SETUP_BINARY)" ]; then \
		$(TARGET_DIR)/$(SETUP_BINARY) docker; \
	else \
		echo "❌ Setup binary not found. Run 'make build' first."; \
		exit 1; \
	fi
	@echo "✅ Docker configuration generated!"

# Build Docker image
docker-build: docker
	@echo "🐳 Building Docker image..."
	docker build -t kg-mcp-server:latest .
	@echo "✅ Docker image built!"

# Run with Docker Compose
docker-up: docker-build
	@echo "🐳 Starting with Docker Compose..."
	docker-compose up -d
	@echo "✅ Server running at http://localhost:8360"

# Stop Docker Compose
docker-down:
	@echo "🐳 Stopping Docker Compose..."
	docker-compose down
	@echo "✅ Docker services stopped!"

# Create release package
package: release
	@echo "📦 Creating release package..."
	@mkdir -p dist
	@tar -czf dist/kg-mcp-server-$(shell uname -s)-$(shell uname -m).tar.gz \
		-C $(TARGET_DIR) $(BINARY_NAME) $(SETUP_BINARY) $(MIGRATE_BINARY) \
		-C ../../ README.md LICENSE install.sh
	@echo "✅ Release package created: dist/kg-mcp-server-$(shell uname -s)-$(shell uname -m).tar.gz"

# Run all quality checks
qa: lint test bench
	@echo "🔍 Quality assurance complete!"

# Validate installation
validate:
	@echo "🔍 Validating installation..."
	@if command -v $(BINARY_NAME) > /dev/null; then \
		echo "✅ $(BINARY_NAME) found in PATH"; \
	elif [ -x "$(INSTALL_DIR)/$(BINARY_NAME)" ]; then \
		echo "✅ $(BINARY_NAME) found in $(INSTALL_DIR)"; \
	else \
		echo "❌ $(BINARY_NAME) not found"; \
		exit 1; \
	fi
	@if [ -f "$(CURSOR_CONFIG_DIR)/mcp.json" ]; then \
		echo "✅ Cursor configuration found"; \
	else \
		echo "⚠️  Cursor configuration not found"; \
	fi
	@echo "✅ Validation complete!"

# Show help
help:
	@echo "🧠 KG MCP Server - Makefile Commands"
	@echo "===================================="
	@echo ""
	@echo "📦 Building & Installation:"
	@echo "  make build       - Build development version"
	@echo "  make release     - Build production release"
	@echo "  make install     - Install binaries and setup Cursor"
	@echo "  make uninstall   - Remove installed binaries"
	@echo "  make setup       - Quick development setup"
	@echo ""
	@echo "🧪 Testing & Quality:"
	@echo "  make test        - Run test suite"
	@echo "  make bench       - Run performance benchmarks"
	@echo "  make lint        - Lint and format code"
	@echo "  make check       - Check code without building"
	@echo "  make qa          - Run all quality checks"
	@echo ""
	@echo "🚀 Running:"
	@echo "  make start       - Start server (development)"
	@echo "  make debug       - Start server (debug mode)"
	@echo ""
	@echo "🐳 Docker:"
	@echo "  make docker      - Generate Docker configuration"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-up   - Start with Docker Compose"
	@echo "  make docker-down - Stop Docker services"
	@echo ""
	@echo "🔧 Utilities:"
	@echo "  make deps        - Install dependencies"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make package     - Create release package"
	@echo "  make validate    - Validate installation"
	@echo "  make help        - Show this help"

# Default help target
.DEFAULT_GOAL := help 