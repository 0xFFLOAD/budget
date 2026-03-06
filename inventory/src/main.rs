use anyhow::{Context, Result};
use chrono::Utc;
use log::{error, info};
use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{env, fs};
use rusqlite::{params, Connection};

// ------------------ types & json utilities ------------------
#[derive(Debug, Serialize, Deserialize)]
struct Product {
    id: Option<i64>,
    name: String,
    price: f64,
    unit: String,
    category_id: i64,
    last_updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Category {
    id: i64,
    name: String,
    parent_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Store {
    id: i64,
    name: String,
    url: String,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T> {
    let bytes = fs::read_to_string(path)?;
    let v = serde_json::from_str(&bytes)?;
    Ok(v)
}

fn write_json<T: Serialize>(path: &str, value: &T) -> Result<()> {
    let data = serde_json::to_string_pretty(value)?;
    fs::write(path, data)?;
    Ok(())
}

// ------------------ configuration ------------------
#[derive(Debug, Serialize, Deserialize)]
struct GeneralConfig {
    url: String,
    max_retries: u32,
    timeout: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SeleniumConfig {
    headless: bool,
    proxy: ProxyConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProxyConfig {
    enabled: bool,
    host: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    path: String,
    cache_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScrapingConfig {
    categories: Vec<String>,
    scrape_interval: u64,
    concurrent_requests: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct TorConfig {
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    general: GeneralConfig,
    selenium: SeleniumConfig,
    database: DatabaseConfig,
    scraping: ScrapingConfig,
    tor: TorConfig,
}

impl Default for Config {
    fn default() -> Self {
        // ensure data directory exists
        let _ = std::fs::create_dir_all("data");

        Config {
            general: GeneralConfig {
                url: "https://www.shufersal.co.il".to_string(),
                max_retries: 3,
                timeout: 30,
            },
            selenium: SeleniumConfig {
                headless: true,
                proxy: ProxyConfig {
                    enabled: true,
                    host: "127.0.0.1".to_string(),
                    port: 9050,
                },
            },
            database: DatabaseConfig {
                path: "data/shufersal_scraper.db".to_string(),
                cache_size: 100,
            },
            scraping: ScrapingConfig {
                categories: vec!["dairy".into(), "produce".into(), "bakery".into()],
                scrape_interval: 60,
                concurrent_requests: 5,
            },
            tor: TorConfig { enabled: false },
        }
    }
}

impl Config {
    fn load() -> Result<Self> {
        // try to read config.json if it exists
        let mut cfg = if let Ok(c) = read_json::<Config>("config.json") {
            c
        } else {
            Config::default()
        };

        // override with environment variables if present
        if let Ok(url) = env::var("SHUFER_Scraper_URL") {
            cfg.general.url = url;
        }
        // user agent ignored when using Tor
        if let Ok(val) = env::var("SHUFER_Scraper_MAX_CONCURRENT") {
            if let Ok(n) = val.parse() {
                cfg.scraping.concurrent_requests = n;
            }
        }
        if let Ok(val) = env::var("SHUFER_Scraper_TOR_ENABLED") {
            if let Ok(b) = val.parse() {
                cfg.tor.enabled = b;
            }
        }
        if let Ok(path) = env::var("SHUFER_Scraper_DATABASE_PATH") {
            cfg.database.path = path;
        }
        // ensure database parent directory exists
        if let Some(parent) = std::path::Path::new(&cfg.database.path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // must have tor enabled
        if !cfg.tor.enabled {
            return Err(anyhow!("configuration requires TOR to be enabled"));
        }
        Ok(cfg)
    }
}

// ------------------ database ------------------
struct Database {
    conn: Connection,
}

impl Database {
    fn init(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL NOT NULL,
                unit TEXT,
                category_id INTEGER,
                last_updated TEXT
            )",
            [],
        )?;
        Ok(Database { conn })
    }

    fn save_product(&self, product: &Product) -> Result<()> {
        self.conn.execute(
            "INSERT INTO products (name, price, unit, category_id, last_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &product.name,
                &product.price,
                &product.unit,
                &product.category_id,
                &product.last_updated
            ],
        )?;
        Ok(())
    }

    fn export_json(&self, path: &str) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT name,price,unit,category_id,last_updated FROM products")?;
        let rows = stmt.query_map([], |row| {
            Ok(Product {
                id: None,
                name: row.get(0)?,
                price: row.get(1)?,
                unit: row.get(2)?,
                category_id: row.get(3)?,
                last_updated: row.get(4)?,
            })
        })?;
        let mut products = Vec::new();
        for r in rows {
            products.push(r?);
        }
        write_json(path, &products)
    }

    fn import_json(&self, path: &str) -> Result<()> {
        let products: Vec<Product> = read_json(path)?;
        for p in products.iter() {
            self.save_product(p)?;
        }
        Ok(())
    }
}

// ------------------ selenium automation (simplified) ------------------
struct SeleniumDriver {
    client: Client,
    last_url: Option<String>,
}

impl SeleniumDriver {
    fn new(cfg: &SeleniumConfig) -> Result<Self> {
        // simple connectivity check to local Selenium server
        let status_url = "http://localhost:4444/status";
        let resp = Client::new().get(status_url).send()?;
        let json: serde_json::Value = resp.json()?;
        if !json["value"]["ready"].as_bool().unwrap_or(false) {
            return Err(anyhow!("selenium server not ready at {}", status_url));
        }
        // browser field not used; assume tor tunnel will handle requests
        let client = Client::builder().build()?;
        Ok(SeleniumDriver { client, last_url: None })
    }

    async fn navigate(&mut self, url: &str) -> Result<()> {
        self.last_url = Some(url.to_string());
        Ok(())
    }

    async fn extract_products(&self) -> Result<Vec<Product>> {
        let url = self
            .last_url
            .as_ref()
            .context("no URL has been navigated to")?;
        let resp = self.client.get(url).send().await?;
        let html = resp.text().await?;
        let fragment = Html::parse_document(&html);
        let selector = Selector::parse(".product-card").unwrap_or_else(|_| Selector::parse("*").unwrap());
        let price_selector = Selector::parse(".price").unwrap_or_else(|_| Selector::parse("*").unwrap());
        let mut products = Vec::new();
        for element in fragment.select(&selector) {
            // extract name (simplified)
            let name = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if name.is_empty() {
                continue; // skip garbage
            }

            // attempt to find a numeric price inside the element or a child
            let mut price: Option<f64> = None;
            if let Some(pel) = element.select(&price_selector).next() {
                let text = pel.text().collect::<Vec<_>>().join(" ");
                if let Some(cap) = regex::Regex::new(r"(\d+[.,]?\d*)")
                    .unwrap()
                    .captures(&text)
                {
                    if let Ok(val) = cap[1].replace(',', ".").parse() {
                        price = Some(val);
                    }
                }
            }
            // fallback: look in full card text
            if price.is_none() {
                if let Some(cap) = regex::Regex::new(r"(\d+[.,]?\d*)")
                    .unwrap()
                    .captures(&name)
                {
                    if let Ok(val) = cap[1].replace(',', ".").parse() {
                        price = Some(val);
                    }
                }
            }
            let price = match price {
                Some(p) if p > 0.0 => p,
                _ => continue, // skip items without valid price
            };

            products.push(Product {
                id: None,
                name,
                price,
                unit: String::new(),
                category_id: 0,
                last_updated: Utc::now().to_rfc3339(),
            });
        }
        Ok(products)
    }
}

// ------------------ scraper logic ------------------
struct Scraper {
    cfg: Config,
    db: Database,
    driver: SeleniumDriver,
}

impl Scraper {
    async fn new(cfg: Config) -> Result<Self> {
        let db = Database::init(&cfg.database.path)?;
        let driver = SeleniumDriver::new(&cfg.selenium)?;
        Ok(Scraper { cfg, db, driver })
    }

    async fn run(&mut self) -> Result<()> {
        for cat in &self.cfg.scraping.categories {
            let url = format!("{}/category/{}", self.cfg.general.url, cat);
            self.driver.navigate(&url).await?;
            let products = self.driver.extract_products().await?;
            // early validation/serialization
            // ensure data directory exists
            let _ = fs::create_dir_all("data");
            write_json("data/latest.json", &products)?;
            for p in &products {
                self.db.save_product(p)?;
            }
        }
        Ok(())
    }
}

// ------------------ error types ------------------
#[derive(thiserror::Error, Debug)]
enum ScraperError {
    #[error("config error: {0}")]
    ConfigError(String),
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("selenium error: {0}")]
    SeleniumError(String),
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("other: {0}")]
    Other(String),
}

impl From<anyhow::Error> for ScraperError {
    fn from(e: anyhow::Error) -> Self {
        ScraperError::Other(e.to_string())
    }
}

// convenience alias
type SResult<T> = std::result::Result<T, ScraperError>;

// ------------------ CLI ------------------
struct CliArgs {
    command: String,
    arg: Option<String>,
}

impl CliArgs {
    fn parse() -> SResult<Self> {
        let mut iter = env::args().skip(1);
        if let Some(cmd) = iter.next() {
            Ok(CliArgs { command: cmd, arg: iter.next() })
        } else {
            Err(ScraperError::ConfigError("no command provided".into()))
        }
    }
}

// ------------------ main ------------------
#[tokio::main]
async fn main() -> SResult<()> {
    env_logger::init();

    let args = match CliArgs::parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error parsing arguments: {}", e);
            eprintln!("usage: <init|scrape|dump-json|load-json> [path]");
            return Err(e);
        }
    };

    let cfg = Config::load().map_err(|e| ScraperError::ConfigError(e.to_string()))?;
    // TOR and Selenium are required by design
    if !cfg.tor.enabled {
        return Err(ScraperError::ConfigError("TOR proxy must be enabled".into()));
    }
    let mut scraper = Scraper::new(cfg).await?;

    match args.command.as_str() {
        "init" => {
            // simply ensure database exists and migrations applied
            info!("initializing database at {}", scraper.cfg.database.path);
            let _ = Database::init(&scraper.cfg.database.path)?;
        }
        "scrape" => {
            info!("starting scrape loop");
            scraper.run().await?;
        }
        "dump-json" => {
            let path = args.arg.as_deref().unwrap_or("data/dump.json");
            let _ = fs::create_dir_all("data");
            scraper.db.export_json(path)?;
            info!("exported products to {}", path);
        }
        "load-json" => {
            let path = args.arg.as_deref().unwrap_or("data/dump.json");
            scraper.db.import_json(path)?;
            info!("imported products from {}", path);
        }
        other => {
            error!("unknown command: {}", other);
            eprintln!("usage: <init|scrape|dump-json|load-json> [path]");
        }
    }

    Ok(())
}
