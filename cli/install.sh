#!/bin/bash
# StateSet Commerce CLI Installer
# Usage: curl -fsSL https://stateset.com/install.sh | bash
#
# This script installs the StateSet Commerce CLI with AI-powered agents
# for orders, inventory, checkout, and returns management.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PACKAGE_NAME="@stateset/cli"
MIN_NODE_VERSION=18
GITHUB_REPO="stateset/stateset-icommerce"

print_banner() {
    echo -e "${BLUE}"
    cat << "EOF"
   _____ _        _       _____      _
  / ____| |      | |     / ____|    | |
 | (___ | |_ __ _| |_ ___| (___   ___| |_
  \___ \| __/ _` | __/ _ \\___ \ / _ \ __|
  ____) | || (_| | ||  __/____) |  __/ |_
 |_____/ \__\__,_|\__\___|_____/ \___|\__|

         Commerce CLI Installer
EOF
    echo -e "${NC}"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Get the current OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     OS=linux;;
        Darwin*)    OS=darwin;;
        MINGW*|MSYS*|CYGWIN*) OS=windows;;
        *)          OS=unknown;;
    esac
    echo $OS
}

# Get the current architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   ARCH=x64;;
        arm64|aarch64)  ARCH=arm64;;
        *)              ARCH=unknown;;
    esac
    echo $ARCH
}

# Check Node.js version
check_node_version() {
    if ! command_exists node; then
        return 1
    fi

    NODE_VERSION=$(node -v | sed 's/v//' | cut -d. -f1)
    if [ "$NODE_VERSION" -lt "$MIN_NODE_VERSION" ]; then
        return 1
    fi
    return 0
}

# Install Node.js if needed
install_node() {
    info "Node.js $MIN_NODE_VERSION+ is required but not found"

    if command_exists nvm; then
        info "Installing Node.js via nvm..."
        nvm install $MIN_NODE_VERSION
        nvm use $MIN_NODE_VERSION
    elif command_exists brew; then
        info "Installing Node.js via Homebrew..."
        brew install node@$MIN_NODE_VERSION
    elif command_exists apt-get; then
        info "Installing Node.js via apt..."
        curl -fsSL https://deb.nodesource.com/setup_${MIN_NODE_VERSION}.x | sudo -E bash -
        sudo apt-get install -y nodejs
    elif command_exists yum; then
        info "Installing Node.js via yum..."
        curl -fsSL https://rpm.nodesource.com/setup_${MIN_NODE_VERSION}.x | sudo bash -
        sudo yum install -y nodejs
    else
        error "Could not install Node.js automatically. Please install Node.js $MIN_NODE_VERSION+ manually:

    https://nodejs.org/en/download/

Or use a version manager:

    # nvm (recommended)
    curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
    nvm install $MIN_NODE_VERSION

Then run this installer again."
    fi
}

# Install the CLI via npm
install_via_npm() {
    info "Installing StateSet CLI via npm..."

    if command_exists sudo && [ "$(id -u)" -ne 0 ]; then
        # Try without sudo first (for nvm users)
        if npm install -g "$PACKAGE_NAME" 2>/dev/null; then
            return 0
        fi
        # Fall back to sudo
        sudo npm install -g "$PACKAGE_NAME"
    else
        npm install -g "$PACKAGE_NAME"
    fi
}

# Verify installation
verify_installation() {
    if ! command_exists stateset; then
        error "Installation failed. 'stateset' command not found."
    fi

    VERSION=$(stateset --version 2>/dev/null || echo "unknown")
    success "StateSet CLI installed: $VERSION"
}

# Print post-install instructions
print_instructions() {
    echo ""
    echo -e "${GREEN}Installation complete!${NC}"
    echo ""
    echo "Available commands:"
    echo "  stateset            - AI-powered commerce operations (auto-routing)"
    echo "  stateset-checkout   - Shopping cart & checkout flow"
    echo "  stateset-orders     - Order lifecycle management"
    echo "  stateset-inventory  - Stock & reservation management"
    echo "  stateset-returns    - RMA & refund processing"
    echo "  stateset-chat       - Interactive multi-turn REPL"
    echo "  stateset-direct     - Direct CLI (no AI)"
    echo ""
    echo "Quick start:"
    echo -e "  ${YELLOW}export ANTHROPIC_API_KEY=sk-ant-...${NC}"
    echo -e "  ${YELLOW}stateset \"show me all customers\"${NC}"
    echo ""
    echo "Documentation:"
    echo "  https://docs.stateset.com/cli"
    echo ""
}

# Main installation flow
main() {
    print_banner

    OS=$(detect_os)
    ARCH=$(detect_arch)
    info "Detected: $OS/$ARCH"

    # Check for Node.js
    if check_node_version; then
        success "Node.js $(node -v) found"
    else
        install_node
        if ! check_node_version; then
            error "Failed to install Node.js"
        fi
        success "Node.js installed"
    fi

    # Check for npm
    if ! command_exists npm; then
        error "npm not found. Please install Node.js with npm."
    fi
    success "npm $(npm -v) found"

    # Install the CLI
    install_via_npm

    # Verify
    verify_installation

    # Show instructions
    print_instructions
}

# Run main
main "$@"
