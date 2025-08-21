# LiteBike - Standard Unix Installation Makefile
# Follows .local/ prefix conventions with proper make install defaults

# Standard installation directories
PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin
MANDIR = $(PREFIX)/share/man/man1
BASHCOMPDIR = $(PREFIX)/share/bash-completion/completions

# Build configuration
CARGO_FLAGS = --release
TARGET_DIR = target/release
BINARY = litebike

# Installation variables
INSTALL = install
INSTALL_PROGRAM = $(INSTALL) -m 755
INSTALL_DATA = $(INSTALL) -m 644

.PHONY: all build test clean install uninstall check doc completion

# Default target
all: build

# Build the release binary
build:
	@echo "🔨 Building LiteBike (release mode)..."
	cargo build $(CARGO_FLAGS)
	@echo "✅ Build complete: $(TARGET_DIR)/$(BINARY)"

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test $(CARGO_FLAGS)

# Check code without building
check:
	@echo "🔍 Checking code..."
	cargo check

# Generate documentation
doc:
	@echo "📚 Generating documentation..."
	cargo doc --no-deps

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -f completion/litebike-completion.bash

# Install to .local/ (standard Unix convention)
install: build completion
	@echo "📦 Installing LiteBike to $(PREFIX)..."
	$(INSTALL) -d $(BINDIR)
	$(INSTALL_PROGRAM) $(TARGET_DIR)/$(BINARY) $(BINDIR)/
	@echo "📁 Creating completion directory..."
	$(INSTALL) -d $(BASHCOMPDIR)
	$(INSTALL_DATA) completion/litebike-completion.bash $(BASHCOMPDIR)/litebike
	@echo "✅ Installation complete!"
	@echo ""
	@echo "🎯 Usage:"
	@echo "   Add $(BINDIR) to your PATH if not already present:"
	@echo "   echo 'export PATH=\"$(BINDIR):\$$PATH\"' >> ~/.bashrc"
	@echo "   source ~/.bashrc"
	@echo ""
	@echo "   Enable bash completion:"
	@echo "   echo 'source $(BASHCOMPDIR)/litebike' >> ~/.bashrc"

# Uninstall from .local/
uninstall:
	@echo "🗑️  Uninstalling LiteBike from $(PREFIX)..."
	rm -f $(BINDIR)/$(BINARY)
	rm -f $(BASHCOMPDIR)/litebike
	@echo "✅ Uninstallation complete!"

# Generate bash completion using DSEL exploration
completion: build
	@echo "🔧 Generating intelligent bash completion..."
	mkdir -p completion
	$(TARGET_DIR)/$(BINARY) completion bash > completion/litebike-completion.bash
	@echo "✅ Completion generated: completion/litebike-completion.bash"

# Install for development (symlink instead of copy)
install-dev: build completion
	@echo "🔗 Installing LiteBike for development (symlinks)..."
	$(INSTALL) -d $(BINDIR)
	$(INSTALL) -d $(BASHCOMPDIR)
	ln -sf $(PWD)/$(TARGET_DIR)/$(BINARY) $(BINDIR)/
	ln -sf $(PWD)/completion/litebike-completion.bash $(BASHCOMPDIR)/litebike
	@echo "✅ Development installation complete!"

# Check installation
check-install:
	@echo "🔍 Checking installation..."
	@if [ -x "$(BINDIR)/$(BINARY)" ]; then \
		echo "✅ Binary found: $(BINDIR)/$(BINARY)"; \
		$(BINDIR)/$(BINARY) version-check || true; \
	else \
		echo "❌ Binary not found at $(BINDIR)/$(BINARY)"; \
		echo "   Run 'make install' to install"; \
	fi
	@if [ -f "$(BASHCOMPDIR)/litebike" ]; then \
		echo "✅ Completion found: $(BASHCOMPDIR)/litebike"; \
	else \
		echo "❌ Completion not found at $(BASHCOMPDIR)/litebike"; \
	fi

# Package preparation (for distribution)
dist-prep: clean build test doc
	@echo "📦 Preparing distribution package..."
	@echo "✅ Distribution ready: $(TARGET_DIR)/$(BINARY)"

# Development workflow targets
dev-build: check test build

dev-test: test check-install

dev-clean: clean
	cargo clippy --fix --allow-dirty --allow-staged || true

# Help target showing available commands
help:
	@echo "🚀 LiteBike Makefile - Standard Unix Installation"
	@echo ""
	@echo "📋 Available targets:"
	@echo "   build         Build release binary"
	@echo "   test          Run all tests"
	@echo "   check         Check code without building"
	@echo "   doc           Generate documentation"
	@echo "   clean         Clean build artifacts"
	@echo ""
	@echo "📦 Installation targets:"
	@echo "   install       Install to ~/.local/ (standard)"
	@echo "   uninstall     Remove from ~/.local/"
	@echo "   install-dev   Install with symlinks for development"
	@echo "   check-install Verify installation"
	@echo ""
	@echo "🔧 Utility targets:"
	@echo "   completion    Generate bash completion"
	@echo "   dist-prep     Prepare for distribution"
	@echo "   help          Show this help"
	@echo ""
	@echo "🎯 Common workflows:"
	@echo "   make && make install     # Build and install"
	@echo "   make install-dev         # Development installation"
	@echo "   make test check-install  # Test and verify"
	@echo ""
	@echo "⚙️  Configuration:"
	@echo "   PREFIX=$(PREFIX)"
	@echo "   BINDIR=$(BINDIR)"
	@echo "   BASHCOMPDIR=$(BASHCOMPDIR)"

# Pattern rules for common typos/alternatives
instal: install
isntall: install
intall: install