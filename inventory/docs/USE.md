# Usage Guide for Shufersal Scraper

This document explains how to build, configure, and run the scraper application described in this repository.

## Prerequisites

Before anything else you can run the bundled helper script (now
located in `scripts/`) which attempts to install all required system
packages on macOS:

```bash
cd /Users/sam/budget/inventory
./scripts/requirements.sh
```

1. **Rust & Cargo**
   - Installed by the script; otherwise install via https://rustup.rs
   - Ensure `cargo` is available in your `PATH`.

2. **Java JDK (8 or 11)**
   - Needed by Selenium WebDriver. The scraper will connect to a local
     Selenium server, so Java must be installed and the driver started.

3. **Browser & WebDriver**
   - Because all scraping must be routed through Tor, the browser field is
     a nominal value (default `tor`). You still need a Selenium server
     running (using any browser binary), but traffic will pass through the
     Tor proxy defined in the config.
   - Start a Selenium standalone server (e.g. `selenium-server -port 4444`).
   - The code will check `http://localhost:4444/status` on startup and
     fail if the service is not reachable.

4. **Tor**
   - Tor proxy **must be running** on `127.0.0.1:9050`; the scraper
     rejects configuration where Tor is disabled. Install via brew
     or your package manager and run `tor` before scraping.


## Building the Project

... (rest unchanged) ...


## Building the Project

From the `inventory` directory:

```bash
cd /Users/sam/budget/inventory
cargo build --release
```

This will compile `src/main.rs` and its dependencies, producing a binary in `target/release/shufersal_scraper`.


## Configuration

Configuration values are loaded in the following order (later entries override earlier ones):

1. **`config.json` file** (a template is provided at the repository root `config.json`).  The file
   must include valid `selenium` and `tor` sections; disabling Tor is not
   permitted and the scraper will refuse to run.
2. **Environment variables**

Default values are defined in the source (see `Config::default()`).

### config.json structure

```json
{
  "general": {
    "url": "https://www.shufersal.co.il",
    "user_agent": "...",
    "max_retries": 3,
    "timeout": 30
  },
  "selenium": {
    "browser": "tor",             # placeholder; all traffic goes through Tor
    "headless": true,
    "proxy": { "enabled": true, "host": "127.0.0.1", "port": 9050 }
  },
  "database": { "path": "data/shufersal_scraper.db", "cache_size": 100 },
  "scraping": {
    "categories": ["dairy","produce","bakery"],
    "scrape_interval": 60,
    "concurrent_requests": 5
  },
  "tor": { "enabled": false }
}
```

### Environment variables

- `SHUFER_Scraper_URL` – base URL (default `https://www.shufersal.co.il`)
- `SHUFER_Scraper_USER_AGENT` – custom UA
- `SHUFER_Scraper_MAX_CONCURRENT` – parallel request count
- `SHUFER_Scraper_TOR_ENABLED` – `true` or `false`
- `SHUFER_Scraper_DATABASE_PATH` – SQLite file path


## Commands

> **Note:** prior to running any command ensure the Selenium server is
> listening at localhost:4444 and the Tor daemon is running.

Usage syntax:

```sh
cargo run -- <command> [path]
# or after building: ./target/release/shufersal_scraper <command> [path]
```

### init

Initializes the SQLite database (creates tables). Run once before scraping.

```bash
cargo run -- init
```

### scrape

Performs a scraping pass over the configured categories, filters out
any non-food or price-less entries (all garbage dropped), stores valid
items in the database, and writes a `data/latest.json` file containing
only those food/price pairs (creates `data/` if missing).

```bash
cargo run -- scrape
```

### dump-json [file]

Exports all products currently in the database to the specified JSON file
(`data/dump.json` by default).

```bash
cargo run -- dump-json products.json
```

### load-json [file]

Reads products from a JSON file (defaults to `data/dump.json`) and imports them into the database.

```bash
cargo run -- load-json products.json
```


## Troubleshooting

- **`cargo` not found:** ensure Rust is installed and `~/.cargo/bin` is in `PATH`.
- **Database errors:** check file permissions and disk space.
- **Element parsing failures:** website structure might have changed;
  update selector logic in `extract_products`.


## Extending or Modifying

- To replace the dummy HTTP scraping with real Selenium automation,
  modify `SeleniumDriver` in `src/main.rs` or split it into
  `selenium.rs`.
- Add new CLI commands by editing the `match` block in `main`.


---

Keep this guide up to date when the CLI or configuration changes.