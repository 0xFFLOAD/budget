use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use log::{error, info, debug, warn};
use reqwest::blocking::Client;
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
    /// base URL or template; if it contains "{lang}" the scraper will
    /// substitute each entry from `scraping.categories` for that token.
    url: String,
    max_retries: u32,
    timeout: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SeleniumConfig {
    headless: bool,
    browser: String,
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
                url: "https://wolt.com/{lang}/isr/tel-aviv/venue/wolt-market-herzliya".to_string(),
                max_retries: 3,
                timeout: 30,
            },
            selenium: SeleniumConfig {
                headless: true,
                browser: "safari".to_string(),
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
                categories: vec!["en".into(), "he".into()],
                scrape_interval: 60,
                concurrent_requests: 5,
            },
            tor: TorConfig { enabled: false }, // kept for compatibility, not used
        }
    }
}

impl Config {
    fn load() -> Result<Self> {
        // try to read config.json if it exists
        let mut cfg = if let Ok(c) = read_json::<Config>("config.json") {
            debug!("loaded configuration file");
            c
        } else if let Err(e) = read_json::<Config>("config.json") {
            warn!("could not read config.json: {} -- using default", e);
            Config::default()
        } else {
            // unreachable but satisfy exhaustiveness
            Config::default()
        };

        // override with environment variables if present
        if let Ok(url) = env::var("SHUFER_Scraper_URL") {
            cfg.general.url = url;
        }
        if let Ok(br) = env::var("SHUFER_Scraper_BROWSER") {
            cfg.selenium.browser = br;
        }
        // user agent ignored when using Tor
        if let Ok(val) = env::var("SHUFER_Scraper_MAX_CONCURRENT") {
            if let Ok(n) = val.parse() {
                cfg.scraping.concurrent_requests = n;
            }
        }
        if let Ok(path) = env::var("SHUFER_Scraper_DATABASE_PATH") {
            cfg.database.path = path;
        }
        // ensure database parent directory exists
        if let Some(parent) = std::path::Path::new(&cfg.database.path).parent() {
            let _ = std::fs::create_dir_all(parent);
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

// ------------------ selenium automation (webdriver) ------------------
struct SeleniumDriver {
    client: Client,
    base: String,
    session_id: String,
}

impl SeleniumDriver {
    fn new(cfg: &SeleniumConfig) -> Result<Self> {
        // check selenium server
        let base = "http://localhost:4444".to_string();
        let status_url = format!("{}/status", base);
        let resp = Client::new().get(&status_url).send()?;
        let json: serde_json::Value = resp.json()?;
        if !json["value"]["ready"].as_bool().unwrap_or(false) {
            // Grid may report not ready if an existing session is still active;
            // attempt to clear the stale session(s) before moving on.
            warn!("selenium status endpoint returned not-ready: {}", json);
            if let Some(nodes) = json["value"]["nodes"].as_array() {
                for node in nodes {
                    if let Some(slots) = node["slots"].as_array() {
                        for slot in slots {
                            if let Some(sess) = slot.get("session") {
                                if let Some(sid) = sess.get("sessionId").and_then(|v| v.as_str()) {
                                    warn!("deleting stale selenium session {}", sid);
                                    let del_url = format!("{}/session/{}", base, sid);
                                    let _ = Client::new().delete(&del_url).send();
                                }
                            }
                        }
                    }
                }
            }
        }

        let builder = Client::builder(); // no proxy support required
        let client = builder.build()?;

        // create session with desired capabilities
        let mut caps = serde_json::json!({
            "capabilities": {"alwaysMatch": {"browserName": cfg.browser.clone()} }
        });
        let sess_resp = client
            .post(&format!("{}/session", base))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&caps)
            .send()?;
        let sess_json: serde_json::Value = sess_resp.json()?;
        debug!("webdriver new session response: {}", sess_json);
        if let Some(msg) = sess_json["value"]["message"].as_str() {
            if msg.contains("Allow remote automation") {
                return Err(anyhow!("Safari remote automation appears disabled; enable 'Allow Remote Automation' under Develop -> Safari"));
            }
        }
        let session_id = sess_json["value"]["sessionId"]
            .as_str()
            .or_else(|| sess_json["sessionId"].as_str())
            .context("no session id returned by webdriver")?
            .to_string();

        Ok(SeleniumDriver { client, base, session_id })
    }

    fn navigate(&mut self, url: &str) -> Result<()> {
        let nav_url = format!("{}/session/{}/url", self.base, self.session_id);
        let _ = self
            .client
            .post(&nav_url)
            .json(&serde_json::json!({"url": url}))
            .send()?;
        Ok(())
    }

    fn page_source(&self) -> Result<String> {
        let src_url = format!("{}/session/{}/source", self.base, self.session_id);
        let resp = self.client.get(&src_url).send()?;
        let json: serde_json::Value = resp.json()?;
        let src = json["value"]
            .as_str()
            .context("no page source returned")?
            .to_string();
        debug!("page source length {}", src.len());
        Ok(src)
    }

    fn extract_products(&self) -> Result<Vec<Product>> {
        let html = self.page_source()?;
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
                if let Some(cap) = Regex::new(r"(\d+[.,]?\d*)")
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
                if let Some(cap) = Regex::new(r"(\d+[.,]?\d*)")
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
        // if no products were found, return a placeholder to verify scraping
        if products.is_empty() {
            products.push(Product {
                id: None,
                name: "<no-products-detected>".to_string(),
                price: 0.0,
                unit: String::new(),
                category_id: 0,
                last_updated: Utc::now().to_rfc3339(),
            });
        }
        if products.is_empty() {
            products.push(Product {
                id: None,
                name: "<no-products-found>".to_string(),
                price: 0.0,
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
    fn new(cfg: Config) -> Result<Self> {
        let db = Database::init(&cfg.database.path)?;
        let driver = SeleniumDriver::new(&cfg.selenium)?;
        Ok(Scraper { cfg, db, driver })
    }

    fn run(&mut self) -> Result<()> {
        for cat in &self.cfg.scraping.categories {
            // build target URL: template substitution if needed,
            // otherwise append "/category/<cat>" as before.
            let url = if self.cfg.general.url.contains("{lang}") {
                self.cfg.general.url.replace("{lang}", cat)
            } else if cat.starts_with("http") {
                cat.clone()
            } else {
                format!("{}/category/{}", self.cfg.general.url, cat)
            };
            info!("navigating to {}", url);
            self.driver.navigate(&url)?;
            let products = self.driver.extract_products()?;
            info!("{} products found for category {}", products.len(), cat);
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
fn main() -> SResult<()> {
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

    // handle _init_ without spinning an async runtime
    if args.command.as_str() == "init" {
        info!("initializing database at {}", cfg.database.path);
        let _ = Database::init(&cfg.database.path)?;
        return Ok(());
    }

    let mut scraper = Scraper::new(cfg)?;

    match args.command.as_str() {
        "scrape" => {
            info!("starting scrape loop");
            scraper.run()?;
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
