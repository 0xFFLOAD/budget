#!/usr/bin/env bash
# requirements.sh
# Install system dependencies needed to build and run the Shufersal scraper.
# This script targets macOS (Homebrew); adjust commands for other platforms.

set -euo pipefail

function ensure_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "installing $1..."
        brew install "$2"
    else
        echo "$1 already installed"
    fi
}

# Homebrew must be installed
if ! command -v brew >/dev/null 2>&1; then
    echo "Homebrew not found. Install it from https://brew.sh/ and re-run this script."
    exit 1
fi

# Rust & Cargo
if ! command -v cargo >/dev/null 2>&1; then
    echo "Installing Rust and Cargo via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    echo "Rust & Cargo already present"
fi

# Java JDK 11
# Homebrew package might install under /opt/homebrew/opt/openjdk@11
ensure_command java openjdk@11
cat <<'NOTE'
After installing Java you may need to add it to your PATH manually:

  export PATH="/opt/homebrew/opt/openjdk@11/bin:$PATH"

Add that line to ~/.zshrc or ~/.bash_profile if the `java` command
is still not found.

You can verify installation with:
  java --version

NOTE

# Selenium standalone server (Java jar)
# Download latest stable 4.x release; change URL if it moves.
if [ ! -f "selenium-server-standalone.jar" ]; then
    echo "Downloading Selenium standalone server..."
    curl -L -o selenium-server-standalone.jar \
      https://github.com/SeleniumHQ/selenium/releases/download/selenium-4.10.0/selenium-server-4.10.0.jar
    # verify it is a jar
    if ! file selenium-server-standalone.jar | grep -q "Java archive"; then
        echo "download failed or produced invalid jar; please check URL or download manually"
        rm -f selenium-server-standalone.jar
        exit 1
    fi
fi

# Chrome (and chromedriver)
ensure_command google-chrome --cask
ensure_command chromedriver chromedriver

# Tor
ensure_command tor tor

# Create data directory and ensure Cargo dependencies are fetched
mkdir -p data
cd "$(dirname "$0")"/.. && cd inventory
cargo fetch

echo "All dependencies are installed. You may still need to start\n  * tor (tor &),\n  * selenium server (java -jar selenium-server-standalone.jar -port 4444)\nbefore running the scraper."
