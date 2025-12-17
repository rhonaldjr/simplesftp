# SimpleSFTP Makefile
# Cross-platform build automation for Linux and macOS

.PHONY: deps build run install clean help

# Detect OS
UNAME_S := $(shell uname -s)

help:
	@echo "SimpleSFTP Build Targets:"
	@echo "  make deps    - Install system dependencies (Linux only)"
	@echo "  make build   - Build the application in release mode"
	@echo "  make run     - Run the application in development mode"
	@echo "  make install - Install dependencies and build"
	@echo "  make clean   - Clean build artifacts"

# Install system dependencies
deps:
ifeq ($(UNAME_S),Linux)
	@echo "Installing Linux dependencies..."
	@if command -v apt-get >/dev/null 2>&1; then \
		echo "Detected Debian/Ubuntu system"; \
		sudo apt-get update; \
		sudo apt-get install -y libgtk-3-dev libayatana-appindicator3-dev libxdo-dev; \
	elif command -v dnf >/dev/null 2>&1; then \
		echo "Detected Fedora/RHEL system"; \
		sudo dnf install -y gtk3-devel libayatana-appindicator-gtk3-devel xdotool-devel; \
	elif command -v pacman >/dev/null 2>&1; then \
		echo "Detected Arch Linux system"; \
		sudo pacman -S --needed gtk3 libayatana-appindicator xdotool; \
	else \
		echo "Unknown package manager. Please install manually:"; \
		echo "  - libgtk-3-dev"; \
		echo "  - libayatana-appindicator3-dev"; \
		echo "  - libxdo-dev"; \
		exit 1; \
	fi
	@echo "Dependencies installed successfully!"
else ifeq ($(UNAME_S),Darwin)
	@echo "macOS detected - no additional dependencies needed"
else
	@echo "Unsupported OS: $(UNAME_S)"
	@exit 1
endif

# Build in release mode
build:
	@echo "Building SimpleSFTP in release mode..."
	cargo build --release
	@echo "Build complete! Binary at: target/release/simplesftp"

# Run in development mode
run:
	@echo "Running SimpleSFTP in development mode..."
	cargo run

# Install dependencies and build
install: deps build
	@echo "Installation complete!"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@echo "Clean complete!"
