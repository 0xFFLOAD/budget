# Quick Start

Execute the following commands *from the `inventory` directory* (or prefix them with `cd inventory &&`).
Before running them start the required services:

```bash
# (Tor is no longer used for the default wolt workflow)

# launch Selenium server (Java + driver)
# by default the scraper tells Selenium to use `safari` on macOS; other
# browsers (chrome, firefox) may require their respective webdriver
# binaries on the PATH.
# *safari users:* open Safari → Preferences → Advanced and enable
# "Show Develop menu in menu bar", then in Develop menu choose
# "Allow Remote Automation" before starting the server.
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


# run a scraping pass (***start the Selenium server first!***)
# you will see the error "selenium server not ready at http://localhost:4444/status"
# if you attempt to scrape without the server listening.
# the browser used by Selenium is configurable – the default is `safari` on
# macOS.  Chrome/Firefox are also supported if the appropriate WebDriver is
# available and `SHUFER_Scraper_BROWSER` is set accordingly.
# the `scraping.categories` array now holds language codes (e.g. "en","he").
# the default config uses both English and Hebrew variants of the template URL.
# to override or disable Tor you can set environment variables or edit
# `config.json`:
#   export SHUFER_Scraper_BROWSER=chrome
#   export SHUFER_Scraper_TOR_ENABLED=false
cargo run -- scrape

# export stored products to JSON (default path: data/dump.json)
cargo run -- dump-json data/dump.json

# import products from JSON
cargo run -- load-json data/dump.json
```

> **Note:** before running any command make sure:
> * a Selenium server is reachable at http://localhost:4444
> * Tor is no longer required for default scraping; any settings are ignored
> * the `general.url` in your config either points to a single site or
>   serves as a template.  when it contains `{lang}` the scraper will insert
>   each string from `scraping.categories` in its place – the default value
>   is
>   `"https://wolt.com/{lang}/isr/tel-aviv/venue/wolt-market-herzliya"`,
>   which will visit both English and Hebrew variants.
> * optionally, set `selenium.browser` in your JSON or
>   `SHUFER_Scraper_BROWSER` env var (defaults to "safari").
