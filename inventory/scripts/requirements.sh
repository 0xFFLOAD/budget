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
ensure_command java openjdk@11

# Selenium standalone server (Java jar)
if [ ! -f "selenium-server-standalone.jar" ]; then
    echo "Downloading Selenium standalone server..."
    curl -L -o selenium-server-standalone.jar https://repo1.maven.org/maven2/org/seleniumhq/selenium/selenium-server/4.0.0/selenium-server-4.0.0.jar
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
