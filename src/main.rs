mod orderbook;
mod strategy;
mod core;
mod connectors;


use url::Url;
use tungstenite::{connect, Message};

use std::thread;

use log::{error, info, warn};
use log4rs;

use crossbeam_channel::{bounded, Sender};
use crate::connectors::deribit::ws_connector::DeribitConnector;

use crate::strategy::risk;


fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    info!("Starting bot...");

    let (market_event_sender, market_event_receiver) = bounded(10);
    let (order_sender, order_receiver) = bounded(10);
    let (raw_data_sender, raw_data_receiver) = bounded(10);
    let (signal_sender, signal_receiver) = bounded(10);
    let (command_sender, command_receiver) = bounded(10);
    let (portfolio_sender, portfolio_receiver) = bounded(10);


    let command_sender_2 = command_sender.clone();

    let connector_handle = thread::spawn(move || {
        let r = DeribitConnector::new(market_event_sender, order_sender, raw_data_sender, command_receiver,  portfolio_sender, command_sender_2);
        r.run();
    });

    let manager_handle = thread::spawn(move || {
        let manager = strategy::order_manager::Manager::new(signal_receiver, order_receiver, portfolio_receiver, command_sender);
        manager.run();
    });

    let strategy_handle = thread::spawn(move || {
        let strategy = strategy::mm::MarketMaker::new(market_event_receiver, signal_sender);
        strategy.run();
    });


    strategy_handle.join();
    manager_handle.join();
    connector_handle.join();

}