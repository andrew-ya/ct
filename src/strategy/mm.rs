use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use crate::core::entities::OrderbookUpdate;
use crate::orderbook::TreeOrderBook;
use crate::strategy::order_manager::OrderPosition;

pub struct MarketMaker {
    orderbook_receiver: Receiver<OrderbookUpdate>,
    signal_sender: Sender<OrderPosition>,
}

impl MarketMaker {
    pub fn new(orderbook_receiver: Receiver<OrderbookUpdate>,
               signal_sender: Sender<OrderPosition>) -> MarketMaker {
        MarketMaker {
            orderbook_receiver,
            signal_sender,
        }
    }

    pub fn run(&self) {
        let mut orderbook = TreeOrderBook::new();

        thread::scope(|s| {
            s.spawn(move || {

                let mut orderbook_iter = self.orderbook_receiver.iter();
                loop {

                    let orderbook_update = orderbook_iter.next().unwrap();

                    orderbook.add_bids(orderbook_update.bids);
                    orderbook.add_asks(orderbook_update.asks);


                    let bid_price = Decimal::from_f64_retain((orderbook.get_nth_bid(2).unwrap().0 + orderbook.get_nth_bid(3).unwrap().0).to_f64().unwrap() / 2.0).unwrap();
                    let ask_price = Decimal::from_f64_retain((orderbook.get_nth_ask(2).unwrap().0 + orderbook.get_nth_ask(3).unwrap().0).to_f64().unwrap() / 2.0).unwrap();


                    let signal = OrderPosition {
                        bid: bid_price,
                        ask: ask_price
                    };

                    self.signal_sender.send(signal).unwrap();
                }
            });
        });
    }
}