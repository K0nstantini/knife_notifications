mod rest_api;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use binance::futures::websockets::{FuturesMarket, FuturesWebsocketEvent, FuturesWebSockets};
use dotenv::dotenv;
use ftx::options::Options;
use ftx::ws::{Channel, Data, Ws};
use futures::StreamExt;

const TIME_CHANGE: u64 = 60_000;
const PRICE_CHANGE: f64 = 0.05;

#[derive(Copy, Clone, Debug)]
struct Prices {
    time: u64,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let mut ws = Ws::connect(Options::default()).await?;

    let futures = rest_api::get_futures().await?;
    let channels: Vec<_> = futures.iter().map(|f| Channel::Trades(f.name.to_owned())).collect();
    ws.subscribe(channels).await?;

    loop {
        let data = ws.next().await.expect("No data received")?;

        match data {
            (Some(s), Data::Trade(trade)) => {
                println!(
                    "\n{:?} {} {} at {} - liquidation = {}",
                    trade.side, trade.size, s, trade.price, trade.liquidation
                );
            }
            _ => panic!("Unexpected data type"),
        }
    }

    // =========================================================================


    let endpoints: Vec<_> = vec!["!markPrice@arr@1s"]
        .into_iter()
        .map(String::from)
        .collect();

    let mut prices: HashMap<String, Vec<Prices>> = HashMap::new();

    let keep_running = AtomicBool::new(true);
    let mut web_socket = FuturesWebSockets::new(|event: FuturesWebsocketEvent| {
        match event {
            FuturesWebsocketEvent::MarkPriceAll(prices_event) => {
                for e in prices_event {
                    let price = f64::from_str(e.mark_price.as_str()).unwrap();
                    let value = Prices { price, time: e.event_time };
                    prices.entry(e.symbol.clone())
                        .and_modify(|p| {
                            p.retain(|v| v.time > e.event_time - TIME_CHANGE);
                            p.push(value);
                            check_change(&p, &e.symbol);
                        }).or_insert(vec![value]);
                }
                // println!("Count coins: {}", prices.iter().count());
            }
            _ => ()
        }

        Ok(())
    });

    loop {
        web_socket.connect_multiple_streams(FuturesMarket::USDM, &endpoints).unwrap();
        if let Err(e) = web_socket.event_loop(&keep_running) {
            println!("Error: {:?}", e);
        }
    }
}

fn check_change(prices: &Vec<Prices>, symbol: &str) {
    let min_price = prices
        .iter()
        .min_by(|x, y| x.price.partial_cmp(&y.price).unwrap())
        .unwrap().price;
    let max_price = prices
        .iter()
        .max_by(|x, y| x.price.partial_cmp(&y.price).unwrap())
        .unwrap().price;
    let diff = (1.0 - min_price / max_price).abs();
    if diff > PRICE_CHANGE {
        println!("{} - {:.2}%", symbol, diff * 100.0);
    }
}
