use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Result};
use binance::websockets::WebSockets;
use binance::ws_model::{CombinedStreamEvent, WebsocketEvent, WebsocketEventUntag};

mod rest_api;

const TIME_CHANGE: u64 = 5_000;
const PRICE_CHANGE: f64 = 0.02;

#[derive(Copy, Clone, Debug)]
struct Prices {
    time: u64,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let markets = rest_api::get_markets().await?;
    println!("Got {} markets", markets.len());
    trades_websocket(markets).await?;
    Ok(())
}


async fn trades_websocket(markets: Vec<String>) -> Result<()> {
    let mut prices: HashMap<String, Vec<Prices>> = HashMap::new();

    let endpoints = markets.iter().map(|s| format!("{}@trade", s.to_lowercase())).collect();
    let keep_running = AtomicBool::new(true);
    let mut web_socket: WebSockets<'_, CombinedStreamEvent<_>> =
        WebSockets::new(|event: CombinedStreamEvent<WebsocketEventUntag>| {
            if let WebsocketEventUntag::WebsocketEvent(we) = &event.data {
                if let WebsocketEvent::Trade(trade) = we {
                    let price = f64::from_str(&trade.price).unwrap();
                    let value = Prices { price, time: trade.event_time };
                    prices
                        .entry(trade.symbol.clone())
                        .and_modify(|p| {
                            p.push(value);
                            price_processing(p, value.price, value.time, &trade.symbol);
                        })
                        .or_insert(vec![value]);
                }
            }
            Ok(())
        });

    web_socket.connect_multiple(endpoints).await?;
    web_socket.event_loop(&keep_running).await?;
    web_socket.disconnect().await?;
    bail!("Trades disconnected");
}


fn price_processing(prices: &mut Vec<Prices>, last_price: f64, last_time: u64, symbol: &str) {
    let mut num = 0;
    let (mut min_price, mut max_price) = (last_price, last_price);
    for (n, Prices { time, price }) in prices.iter().rev().enumerate() {
        if *time < last_time - TIME_CHANGE {
            num = prices.len() - n;
            break;
        }
        match *price {
            p if p < min_price => min_price = p,
            p if p > max_price => max_price = p,
            _ => ()
        }
    }
    let diff = (1.0 - min_price / max_price).abs();
    if diff > PRICE_CHANGE {
        println!("{} - {:.2}%. Last price: {}", symbol, diff * 100.0, last_price);
        prices.clear();
    } else {
        prices.drain(..num);
    }
}

