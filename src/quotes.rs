use anyhow::{anyhow, Result};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

// ===== Structs de resposta da brapi.dev =====

#[derive(Deserialize, Debug)]
struct BrapiResponse {
    results: Vec<BrapiQuoteResult>,
    #[serde(rename = "requestedAt")]
    #[allow(dead_code)]
    requested_at: Option<String>,
}

#[derive(Deserialize, Debug)]
struct BrapiQuoteResult {
    symbol: String,
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: Option<f64>,
}

/// Busca as cotações em tempo real de uma lista de tickers usando a API da brapi.dev.
/// Retorna um HashMap mapeando o ticker (em maiúsculo) para o preço atual em Decimal.
///
/// A brapi.dev suporta tickers brasileiros (PETR4, VALE3) e globais (AAPL, TSLA).
/// É necessário configurar a variável de ambiente `BRAPI_TOKEN` com um token válido.
/// Obtenha o token em: https://brapi.dev/dashboard
pub async fn fetch_quotes(tickers: &[String]) -> Result<HashMap<String, Decimal>> {
    if tickers.is_empty() {
        return Ok(HashMap::new());
    }

    let brapi_token = std::env::var("BRAPI_TOKEN").unwrap_or_default();
    if brapi_token.is_empty() {
        warn!("⚠️ BRAPI_TOKEN não configurado. Cotações não serão carregadas.");
        warn!("   Configure em .env ou obtenha um token em https://brapi.dev/dashboard");
        return Ok(HashMap::new());
    }

    // Normaliza tickers para maiúsculas e remove duplicados
    let mut unique_tickers: Vec<String> = tickers
        .iter()
        .map(|t| t.to_uppercase())
        .collect();
    unique_tickers.sort();
    unique_tickers.dedup();

    let symbols = unique_tickers.join(",");
    let url = format!(
        "https://brapi.dev/api/quote/{}?token={}",
        symbols, brapi_token
    );

    info!("🔍 Buscando cotações para: {}...", symbols);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client
        .get(&url)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        error!("❌ Erro da API brapi.dev: código HTTP {}", status);
        return Err(anyhow!("Erro da API de cotações: {}", status));
    }

    let data: BrapiResponse = response.json().await?;

    let mut quotes = HashMap::new();
    for result in data.results {
        if let Some(price_f64) = result.regular_market_price {
            if let Some(price_decimal) = Decimal::from_f64(price_f64) {
                quotes.insert(result.symbol.to_uppercase(), price_decimal);
            }
        }
    }

    info!("✅ Cotações carregadas com sucesso: {} ativos encontrados.", quotes.len());
    Ok(quotes)
}
