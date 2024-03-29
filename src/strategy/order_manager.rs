use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use rust_decimal::Decimal;
use crossbeam_channel::{Receiver, Sender};
use log::{error, info, warn};
use uuid::Uuid;

use crate::core::entities::{Command, OrderSide};

#[derive(Debug, PartialEq, Eq)]
pub enum TradeDirection {
    Bid,
    Ask,
}

#[derive(Debug)]
pub struct OrderPosition {
    pub bid: Decimal,
    pub ask: Decimal,
}


#[derive(Debug)]
pub enum OrderStatus {
    Open,
    Filled,
    Rejected,
    Cancelled,
    Untriggered,
}

#[derive(Debug)]
pub enum OrderEvent {
    OrderChanged {
        id: String,
        direction: TradeDirection,
        price: Decimal,
        amount: Decimal,
        status: OrderStatus,
        label: String,
    },
    OrderSuccess {
        uuid: Uuid,
    },
}

#[derive(Debug)]
pub struct Order {
    id: String,
    direction: TradeDirection,
    price: Decimal,
    amount: Decimal,
    status: OrderStatus,
    label: String,
}


pub struct Balance {
    pub(crate) balance: Decimal
}

pub struct Manager {
    signal_receiver: Receiver<OrderPosition>,
    orders_receiver: Receiver<OrderEvent>,
    portfolio_receiver: Receiver<Balance>,
    command_sender: Sender<Command>,
}

impl Manager {
    pub fn new(signal_receiver: Receiver<OrderPosition>,
               orders_receiver: Receiver<OrderEvent>,
               portfolio_receiver: Receiver<Balance>,
               command_sender: Sender<Command>) -> Manager {
        Manager {
            signal_receiver,
            orders_receiver,
            portfolio_receiver,
            command_sender,
        }
    }

    pub fn run(&self) {
        let balance = Arc::new(Mutex::new(Decimal::from_f64_retain(0.0).unwrap()));

        let b1 = Arc::clone(&balance);
        let b2 = Arc::clone(&balance);

        let order_count = Arc::new(Mutex::new(0));

        let oc1 = Arc::clone(&order_count);
        let oc2 = Arc::clone(&order_count);

        // todo store order data in arrays/vectors
        let active_orders = Arc::new(Mutex::new(HashMap::<String, Order>::new()));

        let ao1 = Arc::clone(&active_orders);
        let ao2 = Arc::clone(&active_orders);

        let unconfirmed_orders = Arc::new(Mutex::new(HashSet::<Uuid>::new()));
        let uo1 = Arc::clone(&unconfirmed_orders);


        let order_receiver_clone = crossbeam_channel::Receiver::clone(&self.orders_receiver); //
        let portfolio_receiver_clone = self.portfolio_receiver.clone(); //
        let signal_receiver_clone = self.signal_receiver.clone(); //
        let command_sender_clone = crossbeam_channel::Sender::clone(&self.command_sender.clone());

        thread::spawn(move || { // update orders
            let mut orders = order_receiver_clone.iter();
            loop {
                let order_response = orders.next().unwrap();

                info!("Got order update: {:?}", &order_response);

                match order_response {
                    OrderEvent::OrderChanged { id, direction, price, amount, status, label } => {
                        let mut existed_orders = ao1.lock().unwrap();

                        match (*existed_orders).get(id.as_str()) {
                            Some(ord) => {
                                match status {
                                    OrderStatus::Filled | OrderStatus::Cancelled => {
                                        (*existed_orders).remove(id.as_str());
                                    }
                                    smth_else => {
                                        warn!("Got incorrect order state for existed order: {:?}, {:?}", smth_else, ord);
                                    }
                                }
                            }
                            None => {
                                match status {
                                    OrderStatus::Open => {
                                        let order_label = label.clone();

                                        let order = Order {
                                            id: id.clone(),
                                            direction,
                                            price,
                                            amount,
                                            status,
                                            label,
                                        };

                                        (*existed_orders).insert(id, order);
                                        let current_timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_millis();

                                        let round_trip = current_timestamp - order_label.parse::<u128>().unwrap();

                                        info!("Round trip: {}", round_trip);
                                    }
                                    smth_else => {
                                        warn!("Got incorrect order state for non-existed order: {:?}", smth_else);
                                    }
                                };
                            }
                        }

                        info!("existed orders: {:?}", &existed_orders);
                    }

                    OrderEvent::OrderSuccess { uuid } => {
                        let mut unconfirmed = unconfirmed_orders.lock().unwrap();
                        if (*unconfirmed).contains(&uuid) {
                            (*unconfirmed).remove(&uuid);
                        };
                    }
                }
            }
        });


        thread::spawn(move || { // update the balance
            let mut portfolio_iter = portfolio_receiver_clone.iter();

            loop {
                let p = portfolio_iter.next().unwrap();

                let mut existed_balance = b1.lock().unwrap();

                let old_balance = (*existed_balance).clone();
                *existed_balance = p.balance;

                info!("Updated balance - old: {}, current: {}", old_balance, p.balance);
            }
        });


        thread::spawn(move || {
            let mut signal_iter = signal_receiver_clone.iter();

            let instrument = "BTC-PERPETUAL";
            let default_amount = Decimal::from_f64_retain(10.0).unwrap();


            loop {
                let signal = signal_iter.next().unwrap();

                // info!("Got signal {:?} on {:?}",  signal, SystemTime::now());

                // info!("lock unconfirmed");
                let mut unconfirmed = uo1.lock().unwrap();

                if (*unconfirmed).is_empty() {
                    let orders = ao2.lock().unwrap();
                    let balance = b2.lock().unwrap();

                    let bid = (*orders).values().filter(|order| order.direction == TradeDirection::Bid).count();
                    let ask = (*orders).values().filter(|order| order.direction == TradeDirection::Ask).count();

                    if bid == 0 && ask == 0 {
                        let current_timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis();

                        let ask_uuid = Uuid::new_v4();
                        let ask_order = Command::MakeOrder {
                            request_id: ask_uuid.clone(),
                            direction: OrderSide::Ask,
                            instrument: instrument.to_string(),
                            price: signal.ask,
                            amount: default_amount,
                            label: current_timestamp.to_string(),
                        };

                        command_sender_clone.send(ask_order).unwrap();
                        (*unconfirmed).insert(ask_uuid);

                        let bid_uuid = Uuid::new_v4();

                        let bid_order = Command::MakeOrder {
                            request_id: bid_uuid.clone(),
                            direction: OrderSide::Bid,
                            instrument: instrument.to_string(),
                            price: signal.bid,
                            amount: default_amount,
                            label: current_timestamp.to_string(),
                        };

                        command_sender_clone.send(bid_order).unwrap();
                        (*unconfirmed).insert(bid_uuid);
                    }
                } else {
                    info!("unconfirmed {:?}", *unconfirmed);
                }
                // info!("release unconfirmed");
                // thread::sleep(Duration::from_micros(1));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn check_balances() {}
}