#!/usr/bin/env bash
#
# g - Enhanced Git CLI installer
# Usage: curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
#

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

REPO="bdryanovski/g"
BINARY_NAME="g"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
TMP_DIR=""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
DIM='\033[2m'
BOLD='\033[1m'
RESET='\033[0m'

# ─────────────────────────────────────────────────────────────────────────────
# Banner
# ─────────────────────────────────────────────────────────────────────────────

print_banner() {
    echo ""
    echo -e "${MAGENTA}${BOLD}"
    cat << 'EOF'
                                                                        
       ██████╗       ██████╗██╗     ██╗                                
      ██╔════╝      ██╔════╝██║     ██║                                
      ██║  ███╗     ██║     ██║     ██║                                
      ██║   ██║     ██║     ██║     ██║                                
      ╚██████╔╝     ╚██████╗███████╗██║                                
       ╚═════╝       ╚═════╝╚══════╝╚═╝                                
                                                                        
EOF
    echo -e "${RESET}"
    echo -e "${DIM}    Enhanced Git CLI with stacked PRs, workspaces, and beautiful output${RESET}"
    echo ""
}

# ─────────────────────────────────────────────────────────────────────────────
# Progress helpers
# ─────────────────────────────────────────────────────────────────────────────

spinner() {
    local pid=$1
    local message=$2
    local spinstr='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    local i=0
    
    while kill -0 "$pid" 2>/dev/null; do
        local char="${spinstr:$i:1}"
        printf "\r  ${CYAN}%s${RESET} %s" "$char" "$message"
        i=$(( (i + 1) % ${#spinstr} ))
        sleep 0.1
    done
    printf "\r"
}

step() {
    echo -e "  ${GREEN}✓${RESET} $1"
}

step_start() {
    echo -ne "  ${CYAN}○${RESET} $1...\r"
}

step_done() {
    # Clear the line first, then print the result
    echo -ne "\033[2K"
    echo -e "  ${GREEN}✓${RESET} $1"
}

step_fail() {
    echo -e "\r  ${RED}✗${RESET} $1"
}

info() {
    echo -e "  ${BLUE}ℹ${RESET} $1"
}

warn() {
    echo -e "  ${YELLOW}⚠${RESET} $1"
}

error() {
    echo -e "  ${RED}✗${RESET} $1" >&2
}

# ─────────────────────────────────────────────────────────────────────────────
# Requirements check
# ─────────────────────────────────────────────────────────────────────────────

check_requirements() {
    local missing=()

    # Check for git (required)
    if ! command -v git &>/dev/null; then
        missing+=("git")
    fi

    # Check for curl or wget (one required for download)
    if ! command -v curl &>/dev/null && ! command -v wget &>/dev/null; then
        missing+=("curl or wget")
    fi

    # Check for tar or unzip (one required for extraction)
    if ! command -v tar &>/dev/null && ! command -v unzip &>/dev/null; then
        missing+=("tar or unzip")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        echo ""
        error "Missing required dependencies:"
        echo ""
        for dep in "${missing[@]}"; do
            echo -e "    ${RED}•${RESET} $dep"
        done
        echo ""
        echo -e "  Please install the missing dependencies and try again."
        echo ""
        
        # Provide install hints based on OS
        case "$(uname -s)" in
            Linux*)
                echo -e "  ${DIM}On Debian/Ubuntu:${RESET}"
                echo -e "    ${WHITE}sudo apt-get install git curl${RESET}"
                echo ""
                echo -e "  ${DIM}On Fedora/RHEL:${RESET}"
                echo -e "    ${WHITE}sudo dnf install git curl${RESET}"
                echo ""
                echo -e "  ${DIM}On Arch Linux:${RESET}"
                echo -e "    ${WHITE}sudo pacman -S git curl${RESET}"
                ;;
            Darwin*)
                echo -e "  ${DIM}On macOS (using Homebrew):${RESET}"
                echo -e "    ${WHITE}brew install git${RESET}"
                echo ""
                echo -e "  ${DIM}Or install Xcode Command Line Tools:${RESET}"
                echo -e "    ${WHITE}xcode-select --install${RESET}"
                ;;
        esac
        echo ""
        exit 1
    fi

    # Show git version
    local git_version
    git_version=$(git --version | awk '{print $3}')
    step "Git found: ${CYAN}v${git_version}${RESET}"
}

# ─────────────────────────────────────────────────────────────────────────────
# Platform detection
# ─────────────────────────────────────────────────────────────────────────────

detect_platform() {
    local os arch target

    # Detect OS
    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)
            error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)
            error "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac

    # Build target triple
    case "$os" in
        linux)   target="${arch}-unknown-linux-gnu" ;;
        darwin)  target="${arch}-apple-darwin" ;;
        windows) target="${arch}-pc-windows-msvc" ;;
    esac

    echo "$target"
}

detect_archive_ext() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*) echo "zip" ;;
        *) echo "tar.gz" ;;
    esac
}

# ─────────────────────────────────────────────────────────────────────────────
# Download helpers
# ─────────────────────────────────────────────────────────────────────────────

get_latest_version() {
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget &>/dev/null; then
        wget -qO- "$url" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
        exit 1
    fi
}

download_file() {
    local url="$1"
    local dest="$2"
    
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget &>/dev/null; then
        wget -q "$url" -O "$dest"
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Installation
# ─────────────────────────────────────────────────────────────────────────────

install() {
    print_banner

    echo -e "${BOLD}Installing ${BINARY_NAME}...${RESET}"
    echo ""

    # Check requirements first
    check_requirements

    # Detect platform
    step_start "Detecting platform"
    local target
    target=$(detect_platform)
    local ext
    ext=$(detect_archive_ext)
    step_done "Detected ${CYAN}${target}${RESET}"

    # Get latest version
    step_start "Fetching latest version"
    local version
    version=$(get_latest_version)
    if [ -z "$version" ]; then
        step_fail "Could not determine latest version"
        error "Failed to fetch release information from GitHub"
        exit 1
    fi
    step_done "Latest version: ${CYAN}${version}${RESET}"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    # Download archive
    local archive_name="${BINARY_NAME}-${target}.${ext}"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"
    
    step_start "Downloading ${archive_name}"
    if ! download_file "$download_url" "$TMP_DIR/$archive_name" 2>/dev/null; then
        step_fail "Download failed"
        error "Could not download: $download_url"
        error "Please check if the release exists for your platform."
        exit 1
    fi
    step_done "Downloaded ${CYAN}${archive_name}${RESET}"

    # Extract archive
    step_start "Extracting archive"
    cd "$TMP_DIR"
    if [ "$ext" = "tar.gz" ]; then
        tar -xzf "$archive_name"
    else
        unzip -q "$archive_name"
    fi
    step_done "Extracted successfully"

    # Create install directory
    step_start "Installing to ${INSTALL_DIR}"
    mkdir -p "$INSTALL_DIR"
    
    # Copy binary
    if [ -f "$BINARY_NAME" ]; then
        cp "$BINARY_NAME" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    elif [ -f "${BINARY_NAME}.exe" ]; then
        cp "${BINARY_NAME}.exe" "$INSTALL_DIR/"
    else
        step_fail "Binary not found in archive"
        exit 1
    fi
    step_done "Installed to ${CYAN}${INSTALL_DIR}/${BINARY_NAME}${RESET}"

    echo ""
    echo -e "${GREEN}${BOLD}Installation complete!${RESET}"
    echo ""

    # Check if in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        print_shell_setup
    else
        # Check if another g binary takes precedence
        local which_g
        which_g=$(command -v g 2>/dev/null || true)
        if [ -n "$which_g" ] && [ "$which_g" != "$INSTALL_DIR/$BINARY_NAME" ]; then
            warn "Another 'g' binary found at: ${CYAN}${which_g}${RESET}"
            warn "The installed binary is at: ${CYAN}${INSTALL_DIR}/${BINARY_NAME}${RESET}"
            echo ""
            echo -e "  To use the newly installed version, either:"
            echo -e "    1. Remove the other binary: ${WHITE}rm ${which_g}${RESET}"
            echo -e "    2. Or add ${CYAN}${INSTALL_DIR}${RESET} earlier in your PATH"
            echo ""
        else
            echo -e "  Run ${CYAN}${BOLD}g --help${RESET} to get started!"
            echo ""
        fi
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Shell setup instructions
# ─────────────────────────────────────────────────────────────────────────────

print_shell_setup() {
    echo -e "${YELLOW}${BOLD}Shell Setup Required${RESET}"
    echo ""
    echo -e "  Add ${CYAN}${INSTALL_DIR}${RESET} to your PATH:"
    echo ""
    
    # Detect shell
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/bash}")
    
    case "$shell_name" in
        bash)
            echo -e "  ${DIM}# Add to ~/.bashrc or ~/.bash_profile${RESET}"
            echo -e "  ${WHITE}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}"
            echo ""
            echo -e "  ${DIM}# Then reload:${RESET}"
            echo -e "  ${WHITE}source ~/.bashrc${RESET}"
            ;;
        zsh)
            echo -e "  ${DIM}# Add to ~/.zshrc${RESET}"
            echo -e "  ${WHITE}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}"
            echo ""
            echo -e "  ${DIM}# Then reload:${RESET}"
            echo -e "  ${WHITE}source ~/.zshrc${RESET}"
            ;;
        fish)
            echo -e "  ${DIM}# Add to ~/.config/fish/config.fish${RESET}"
            echo -e "  ${WHITE}fish_add_path \$HOME/.local/bin${RESET}"
            echo ""
            echo -e "  ${DIM}# Then reload:${RESET}"
            echo -e "  ${WHITE}source ~/.config/fish/config.fish${RESET}"
            ;;
        *)
            echo -e "  ${DIM}# Add to your shell's config file${RESET}"
            echo -e "  ${WHITE}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}"
            ;;
    esac
    
    echo ""
    echo -e "${BOLD}Shell Completions${RESET}"
    echo ""
    
    case "$shell_name" in
        bash)
            echo -e "  ${DIM}# Generate and install completions${RESET}"
            echo -e "  ${WHITE}${BINARY_NAME} completions bash >> ~/.bash_completion${RESET}"
            ;;
        zsh)
            echo -e "  ${DIM}# Generate and install completions${RESET}"
            echo -e "  ${WHITE}mkdir -p ~/.zsh/completions${RESET}"
            echo -e "  ${WHITE}${BINARY_NAME} completions zsh > ~/.zsh/completions/_${BINARY_NAME}${RESET}"
            echo ""
            echo -e "  ${DIM}# Add to ~/.zshrc if not already present${RESET}"
            echo -e "  ${WHITE}fpath=(~/.zsh/completions \$fpath)${RESET}"
            echo -e "  ${WHITE}autoload -Uz compinit && compinit${RESET}"
            ;;
        fish)
            echo -e "  ${DIM}# Generate and install completions${RESET}"
            echo -e "  ${WHITE}${BINARY_NAME} completions fish > ~/.config/fish/completions/${BINARY_NAME}.fish${RESET}"
            ;;
    esac
    
    echo ""
    echo -e "  ${DIM}After setup, run ${CYAN}${BOLD}g --help${RESET}${DIM} to get started!${RESET}"
    echo ""
}

# ─────────────────────────────────────────────────────────────────────────────
# Uninstall
# ─────────────────────────────────────────────────────────────────────────────

uninstall() {
    print_banner
    
    echo -e "${BOLD}Uninstalling ${BINARY_NAME}...${RESET}"
    echo ""
    
    local binary_path="$INSTALL_DIR/$BINARY_NAME"
    
    if [ -f "$binary_path" ]; then
        step_start "Removing $binary_path"
        rm -f "$binary_path"
        step_done "Removed binary"
        echo ""
        echo -e "${GREEN}${BOLD}Uninstall complete!${RESET}"
        echo ""
        info "You may also want to remove shell completions and PATH entries."
    else
        warn "${BINARY_NAME} is not installed in ${INSTALL_DIR}"
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
    case "${1:-install}" in
        install)
            install
            ;;
        uninstall|remove)
            uninstall
            ;;
        --help|-h)
            echo "Usage: $0 [install|uninstall]"
            echo ""
            echo "Environment variables:"
            echo "  INSTALL_DIR    Installation directory (default: ~/.local/bin)"
            ;;
        *)
            error "Unknown command: $1"
            echo "Usage: $0 [install|uninstall]"
            exit 1
            ;;
    esac
}

main "$@"
