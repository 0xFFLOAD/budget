# Shufersal Scraper

A Rust-based web scraper designed to extract product data from Shufersal (and similar) websites, mapping item names to their prices for a personal budgeting application.

## Overview

This application leverages Rust's performance and safety guarantees combined with browser automation (Selenium) to scrape dynamic, JavaScript-heavy websites. **A running Selenium server and Tor proxy are required for the tool to function; it performs a startup check and will abort otherwise.** The scraper will filter the page content and keep only food-related products that have a valid numeric price; all other text or elements are considered garbage and ignored. The scraped data is stored in an SQLite database for efficient querying and retrieval.

## Key Features

- Browser automation using Selenium to handle JavaScript rendering
- JSON serialization/deserialization for configuration and data exchange
- SQLite database integration for persistent storage
- Asynchronous I/O operations using Tokio
- HTTP client capabilities with Reqwest
- Comprehensive error handling and logging
- Configuration management for flexible operation

## Architecture Overview

The application consists of several core components:

1. **Browser Automation:** an external Selenium server handles interaction with the target website through a real browser instance
2. **Data Storage:** rusqlite provides a safe interface to SQLite for persistent storage; all exported or intermediate data is structured as JSON (not TSV)
3. **Data Handling:** serde-json enables JSON processing for configuration and data exchange
4. **Networking:** reqwest powers HTTP requests for initial page loads and API interactions
5. **Asynchronous Runtime:** tokio provides the foundation for all async operations

## Dependencies

```toml
[package]
name = "shufersal_scraper"
version = "0.1.0"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rusqlite = "0.20"
scraper = "0.8"
anyhow = "1.0"
thiserror = "1.0"
log = "0.4"
env_logger = "0.9"
config = "0.12"
chrono = "0.4"
uuid = "0.8"
```

## Prerequisites

Before building and running the application, ensure you have the following installed and running:

1. **Java JDK (version 8 or 11):**
   - Selenium requires Java to be installed on your system
   - For Ubuntu/Debian:
     ```bash
     sudo apt update
     sudo apt install openjdk-11-jdk
     ```
   - For macOS:
     ```bash
     brew install adoptopenjdk11
     ```
   - For Windows:
     Download from [Oracle's Java download page](https://www.oracle.com/java/technetwork/java/javase/downloads/index.html)

2. **Rust and Cargo:**
   - Install from [rustup.rs](https://rustup.rs/)

3. **Browser & WebDriver:**
   - A Selenium server must be running (e.g. using Chrome, Firefox, or any
     other browser), but the configuration no longer requires specifying a
     browser name. All traffic is routed through Tor so the browser value is
     ignored.
   - Run the Selenium standalone server (typically listening on http://localhost:4444).
     The scraper will check this endpoint at startup and fail if unreachable.

4. **Tor:**
   - Install Tor:
     - For Ubuntu/Debian:
       ```bash
       sudo apt install tor
       ```
     - For macOS:
       ```bash
       brew install tor
       ```
   - **Tor must be running** before scraping, e.g. `tor &`.

## Database Initialization

The application uses SQLite for data storage. The database schema is defined in the `migrations` directory using Rusqlite's migration system.

### Initializing the Database

Run the following command to initialize the database:

```bash
cargo run -- init
```

This will:
1. Create a new SQLite database file (`shufersal_scraper.db`)
2. Apply database migrations
3. Create necessary tables (products, categories, etc.)

### Database Structure

The database schema includes the following tables:

1. **Products:**
   - id (INTEGER PRIMARY KEY)
   - name (TEXT)
   - price (REAL)
   - unit (TEXT)
   - category_id (INTEGER)
   - last_updated (DATETIME)

> The SQLite file and JSON exports live under a `data/` directory by default.

2. **Categories:**
   - id (INTEGER PRIMARY KEY)
   - name (TEXT)
   - parent_id (INTEGER)

3. **Stores:**
   - id (INTEGER PRIMARY KEY)
   - name (TEXT)
   - url (TEXT)

## Configuration

The application uses the `config` crate for configuration management. Configuration can be loaded from environment variables, command-line arguments, and JSON configuration files.

### Configuration File

Create a `config.json` file with the following structure:

```json
{
  "general": {
    "url": "https://www.shufersal.co.il",
    "max_retries": 3,
    "timeout": 30
  },
  "selenium": {
    "browser": "chrome",
    "headless": true,
    "proxy": {
      "enabled": true,
      "host": "127.0.0.1",
      "port": 9050
    }
  },
  "database": {
    "path": "shufersal_scraper.db",
    "cache_size": 100
  },
  "scraping": {
    "categories": ["dairy", "produce", "bakery"],
    "scrape_interval": 60,
    "concurrent_requests": 5
  }
}
```

### Environment Variables

Available environment variables:

- `SHUFER_Scraper_URL`: Base URL for Shufersal (default: "https://www.shufersal.co.il")
- `SHUFER_Scraper_MAX_CONCURRENT`: Maximum number of concurrent requests (default: 5)
- `SHUFER_Scraper_TOR_ENABLED`: Enable Tor proxy (default: false)  # not required for wolt scraping
- `SHUFER_Scraper_DATABASE_PATH`: SQLite database path (default: "shufersal_scraper.db")

## Usage Examples

### Basic Scrape

```rust
use scraper::Html;
use anyhow::Context;
use anyhow::Result;
use chrono::Utc;

async fn scrape_product_page(url: &str) -> Result<Vec<Product>> {
    let response = reqwest::get(url).await?;
    let html = response.text().await?;
    
    let fragment = Html::parse(html);
    
    let products = fragment.select(scraper::Selector::Class("product-card"))
        .map(|element| Product::parse_element(&element))
        .collect::<Result<Vec<_>>>()
        .context("Failed to parse products")?;
    
    Ok(products)
}
```

### Database Interaction

```rust
use rusqlite::{Connection, Result};

async fn save_products(conn: &Connection, products: &[Product]) -> Result<()> {
    conn.execute(
        "INSERT INTO products (name, price, unit, category_id, last_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
        params!(products.iter().map(|p| (p.name.to_string(), p.price, p.unit.to_string(), p.category_id, Utc::now())))
    )?;
    
    Ok(())
}
```

### Configuration Management

```rust
use config::Config;

fn load_config() -> Result<Config> {
    let config = Config::new()
        .with_default("url", "https://www.shufersal.co.il")
        .with_env_vars()
        .with_json("config.json");
        
    config.try_into_map()
}
```

## Advanced Configuration

The application supports extensive configuration through environment variables and a JSON configuration file. Key settings include:

- `MAX_CONCURRENT_REQUESTS`: Limits the number of parallel scraping operations
- `TOR_ENABLED`: Enables Tor proxy routing (requires Tor installed)  # mostly unused
- `DATABASE_PATH`: Custom SQLite database location
- `SCRAPE_INTERVAL`: Frequency of automatic scraping (in minutes)

## Running the Application

### Basic Usage

```bash
cargo run -- scrape
```

### With Configuration File

```bash
cargo run -- --config config/production.json
```

### With Environment Variables

```bash
export SHUFER_Scraper_URL="https://www.shufersal.co.il"
# export SHUFER_Scraper_TOR_ENABLED=true  (not needed for default config)
cargo run -- scrape
```

## Troubleshooting

Common issues and solutions:

1. **Java not found:**
   - Ensure Java is installed and available in your PATH
   - For macOS: `export PATH=$PATH:/Library/Java/JavaVirtualMachine/Contents/Home/bin`

2. **Selenium not connecting:**
   - Ensure the browser is properly installed
   - Check browser compatibility with your Selenium version
   - Try running with `--headless` to enable headless mode

3. **Database errors:**
   - Ensure write permissions for the database file
   - Check disk space on the device

4. **Element not found:**
   - The website structure might have changed
   - Update the CSS selectors in your scraping code
   - Try using more specific selectors

## Contributing

Contributions are welcome! Please open issues for bug reports or feature requests. When submitting pull requests, please ensure:

- All changes are covered by tests
- Documentation is updated
- Proper error handling is maintained
- The code follows Rust best practices

## License

MIT

## Acknowledgments

- Selenium: Powerful browser automation
- Reqwest: Robust HTTP client
- Rusqlite: Safe SQLite interface
- Serde: Elegant data serialization
- Tokio: Reliable async runtime