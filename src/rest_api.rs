use anyhow::Result;
use binance::api::Binance;
use binance::market::Market;
use binance::rest_model::{Prices, SymbolPrice};

pub async fn get_markets() -> Result<Vec<String>> {
    let market: Market = Binance::new(None, None);
    let exclude = vec![
        "FISUSDT",
        "COSUSDT",
    ];

    let Prices::AllPrices(prices) = market.get_all_prices().await?;
    let prices = prices
        .iter()
        .filter(|p: &&SymbolPrice| p.symbol.ends_with("USDT") && !exclude.contains(&p.symbol.as_str()))
        .map(|p: &SymbolPrice| p.symbol.clone())
        .collect();
    Ok(prices)
}