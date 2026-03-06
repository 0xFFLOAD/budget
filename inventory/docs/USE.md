# Quick Start

Execute the following commands from the `inventory` directory.  Before running them start the required services:

```bash
# start Tor
brew services start tor   # macos
sudo systemctl start tor  # linux

# launch Selenium (requires Java + driver)
 # ensure you have a valid selenium-server-standalone.jar in this directory;
 # the requirements script attempts to fetch one, but you can also
 # download it manually from https://github.com/SeleniumHQ/selenium/releases
 curl -L -o selenium-server-standalone.jar \
  https://github.com/SeleniumHQ/selenium/releases/download/selenium-4.10.0/selenium-server-4.10.0.jar
 java -jar selenium-server-standalone.jar -port 4444 &
```

```bash
# install prerequisites on macOS
chmod +x ./scripts/requirements.sh
./scripts/requirements.sh

# ensure Rust/Cargo is installed and available in PATH
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found: installing Rust using rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
    echo "restart your shell or run 'source $HOME/.cargo/env' to load cargo"
    echo "# if you open a new terminal, remember to source that file again before using cargo"
fi

# source cargo
source "$HOME/.cargo/env"

# build the scraper
cargo build --release

# initialize the database once
cargo run -- init

# install java
brew install openjdk@11

# launch Selenium (requires Java + driver)
# note: include the `standalone` keyword and use the proper port flag
java -jar selenium-server-standalone.jar standalone --port 4444 &

# verify listening
curl http://localhost:4444/status

# see if the tor binary is running
pgrep -lf tor

# see if port 9050 is open locally
lsof -i TCP:9050

# ask the proxy itself for a version string
curl --socks5-hostname 127.0.0.1:9050 http://check.torproject.org/ 2>/dev/null | head -n1

# run a scraping pass (Tor + Selenium must be running)
cargo run -- scrape

# export stored products to JSON (default path: data/dump.json)
cargo run -- dump-json data/dump.json

# import products from JSON
cargo run -- load-json data/dump.json
```

> **Note:** before running any command make sure:
> * a Selenium server is reachable at http://localhost:4444
> * a Tor daemon is running and listening on 127.0.0.1:9050
