# Shufersal Scraper Blueprint

## Overview
A Rust-based web scraper to extract product data from Shufersal websites, mapping item names to their prices for a personal budgeting application. All scraped or exported data is represented as JSON objects (TSV format is intentionally avoided).

> **Prerequisite:** this project requires a live Selenium server (e.g. on localhost:4444) and a running Tor proxy. Startup routines validate connectivity and will error out if either is missing.
## Dependencies
- Rust + Cargo
- Tokio (async runtime)
- Reqwest (HTTP client)
- Serde (JSON serialization/deserialization)
- Rusqlite (SQLite database interface)
- Selenium-rust (browser automation)
- Scraper (HTML parsing)
- anyhow (error handling)
- thiserror (custom error types)
- log + env_logger (logging)
- config (configuration management)
- chrono (date/time handling)
- uuid (unique identifiers)
- Tor (required proxy for scraping)


## JSON Handling & Utilities
Before any TSV-like format is considered, the project treats all structured data as JSON from day one. Several utility modules and patterns are planned:

- **`types.rs`** – definitions of serializable structs (`Product`, `Category`, `Store`, etc.) with `serde::Serialize` / `Deserialize` derived.
- **`json_utils.rs`** – helper functions for reading/writing JSON files, pretty-printing, and merging with config values.

- **Selenium/Tor validation** – initialization code contacts `http://localhost:4444/status` and insists `tor.enabled` is true; configuration without these components is rejected.
- **Export/Import CLI commands** – `dump-json` and `load-json` commands that produce/consume JSON files, used for debugging and data interchange.
- **Database bridges** – converters between `rusqlite::Row` and the JSON-compatible struct types, ensuring data can flow in/out of the DB without ever touching TSV.
- **Early validation** – when scraping, the first step after extraction is to serialize to JSON and validate schema, allowing easier unit testing.

These utilities are referenced in the pseudocode below where applicable.

## Project Structure
```
├── Cargo.toml
├── src/
│   ├── main.rs (entry point)
│   ├── config.rs (configuration module)
│   ├── db.rs (database interactions)
│   ├── scraper.rs (scraper logic)
│   ├── selenium.rs (Selenium automation)
│   ├── errors.rs (error handling)
│   └── cli.rs (command-line interface)
└── data/                            # directory created at runtime
    ├── shufersal_scraper.db         # SQLite database (default)
    ├── latest.json                  # most recent scrape output
    └── dump.json                    # example export file
```

## Pseudocode

### 1. Configuration
```rust
// src/config.rs
struct Config {
    url: String,
    user_agent: String,
    max_retries: u32,
    timeout: u64,
    selenium: SeleniumConfig,
    database: DatabaseConfig,
    scraping: ScrapingConfig,
    tor: TorConfig,
}

impl Config {
    fn load_from_env() -> Result<Self, anyhow::Error> {
        // Load configuration from environment variables
    }
}
```

### 2. Database Module
```rust
// src/db.rs
struct Database {
    conn: Option<rusqlite::Connection>,
}

impl Database {
    fn init(path: &str) -> Result<(), anyhow::Error> {
        // Initialize SQLite database with migrations
    }

    fn save_product(&self, product: &Product) -> Result<(), anyhow::Error> {
        // Insert product into database
    }
}
```

### 3. Selenium Automation
```rust
// src/selenium.rs
struct SeleniumDriver {
    // WebDriver instance
}

impl SeleniumDriver {
    fn new(config: &SeleniumConfig) -> Result<Self, anyhow::Error> {
        // Initialize Selenium WebDriver
    }

    fn navigate(&self, url: &str) -> Result<(), anyhow::Error> {
        // Navigate browser to URL
    }

    fn extract_products(&self) -> Result<Vec<Product>, anyhow::Error> {
        // Extract product data from page, then filter out any items that
        // don't look like food or lack a numeric price; only valid
        // food/price pairs are returned.
    }
}
```

### 4. Scraping Logic
```rust
// src/scraper.rs
struct Scraper {
    config: Config,
    db: Database,
    driver: SeleniumDriver,
}

impl Scraper {
    fn new(config: Config) -> Result<Self, anyhow::Error> {
        // Initialize scraper components
    }

    fn run(&self) -> Result<(), anyhow::Error> {
        // Main scraping loop
    }
}
```

### 5. Error Handling
```rust
// src/errors.rs
#[derive(Debug)]
pub enum ScraperError {
    ConfigError(String),
    DatabaseError(rusqlite::Error),
    SeleniumError(String),
    NetworkError(reqwest::Error),
    // Add other error variants as needed
}

impl From<reqwest::Error> for ScraperError {
    fn from(error: reqwest::Error) -> Self {
        ScraperError::NetworkError(error)
    }
}
```

### 6. CLI Interface
```rust
// src/cli.rs
struct CliArgs {
    command: Option<String>,
    config_path: Option<String>,
}

impl CliArgs {
    fn parse_args() -> Result<Self, anyhow::Error> {
        // Parse command-line arguments
    }
}
```

### 7. Main Function
```rust
// src/main.rs
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = CliArgs::parse_args()?;
    let config = Config::load_from_env()?;

    let db = Database::init(&config.database.path)?;

    let mut scraper = Scraper::new(config)?;

    if let Some(command) = args.command {
        match command.as_str() {
            "init" => {
                // Initialize database schema
            }
            "scrape" => {
                scraper.run().await
            }
            _ => {
                // Handle unknown command
            }
        }
    }
}
```

## Features
- Configuration management (environment variables, JSON config file)
- SQLite database integration with migration support
- Selenium-based browser automation for JavaScript rendering (only food items with a price are kept)
- JSON serialization/deserialization using Serde
- Comprehensive error handling with custom error types
- Logging functionality for debugging and monitoring
- Command-line interface for user interaction
- Tor integration for enhanced privacy (required for scraping)