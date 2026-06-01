//! creem.io HTTP client and transaction parsing.
//!
//! This module isolates everything that depends on the (sparsely documented)
//! creem API shape. Verified against https://docs.creem.io:
//! - `GET /v1/transactions/search` returns `{ "items": [...], "pagination": {...} }`
//! - transaction `status` includes `"paid"`; `created_at` is Unix epoch **seconds**
//! - auth via the `x-api-key` header; amounts are in minor units (cents)

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

const PROD_BASE: &str = "https://api.creem.io";
const TEST_BASE: &str = "https://test-api.creem.io";

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub amount_paid: Option<i64>,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Unix epoch **seconds**.
    #[serde(default)]
    pub created_at: i64,
    // Parsed for resilience / future use (menu detail), not all read yet.
    #[serde(default)]
    #[allow(dead_code)]
    pub customer: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub order: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
}

#[derive(Debug)]
pub enum CreemError {
    /// 401 / 403 — missing or invalid API key.
    Unauthorized,
    /// 429 — rate limited; optional Retry-After seconds.
    RateLimited(Option<u64>),
    Network(String),
    Decode(String),
}

impl fmt::Display for CreemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreemError::Unauthorized => write!(f, "Invalid API key"),
            CreemError::RateLimited(_) => write!(f, "Rate limited"),
            CreemError::Network(e) => write!(f, "Network error: {e}"),
            CreemError::Decode(e) => write!(f, "Unexpected response: {e}"),
        }
    }
}

pub struct CreemClient {
    http: reqwest::Client,
    base: &'static str,
    api_key: String,
}

impl CreemClient {
    pub fn new(api_key: String, test_mode: bool) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            http,
            base: if test_mode { TEST_BASE } else { PROD_BASE },
            api_key,
        }
    }

    /// Fetch the most recent transactions (newest first per the API/CLI).
    pub async fn search_transactions(&self, page_size: u32) -> Result<Vec<Transaction>, CreemError> {
        let url = format!(
            "{}/v1/transactions/search?page_number=1&page_size={}",
            self.base, page_size
        );
        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .map_err(|e| CreemError::Network(e.to_string()))?;

        let code = resp.status().as_u16();
        if code == 401 || code == 403 {
            return Err(CreemError::Unauthorized);
        }
        if code == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());
            return Err(CreemError::RateLimited(retry));
        }
        if !resp.status().is_success() {
            return Err(CreemError::Network(format!("HTTP {code}")));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| CreemError::Network(e.to_string()))?;

        let value: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| CreemError::Decode(e.to_string()))?;

        // Tolerant envelope: documented key is `items`, but accept fallbacks.
        let items = value
            .get("items")
            .or_else(|| value.get("data"))
            .or_else(|| value.get("transactions"))
            .ok_or_else(|| CreemError::Decode("no items array in response".to_string()))?;

        serde_json::from_value::<Vec<Transaction>>(items.clone())
            .map_err(|e| CreemError::Decode(e.to_string()))
    }
}
