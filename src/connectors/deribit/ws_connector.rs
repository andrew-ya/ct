use std::net::TcpStream;
use url::Url;
use tungstenite::{connect, Message, WebSocket};
use crossbeam_channel::{bounded, Sender, Receiver};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use tungstenite::stream::MaybeTlsStream;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::Thread;
use std::time::Duration;
use rust_decimal::Decimal;
use crossbeam_utils::thread as cbu_thread;

use log::{error, info, warn};
use uuid::Uuid;
use crate::connectors::deribit::protocol::*;
use crate::strategy::order_manager;
use crate::core::entities::{Command, OrderSide, PriceLevelAction, PriceLevelChange};
use crate::strategy::order_manager::{Balance, OrderStatus};
use crate::core::entities::OrderbookUpdate;


pub struct DeribitConnector {
    orderbook_sender: Sender<OrderbookUpdate>,
    order_sender: Sender<order_manager::OrderEvent>,
    raw_data_sender: Sender<String>,
    command_receiver: Receiver<Command>,
    socket: Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>,
    portfolio_sender: Sender<Balance>,
    command_sender: Sender<Command>,
}

impl DeribitConnector {
    pub fn run(&self) {
        self.set_heartbeat_interval(60);

        self.authorize();

        self.subscribe_to_orders(vec!("user.orders.BTC-PERPETUAL.raw".into()));
        self.subscribe_to_portfolio_channel(vec!("user.portfolio.btc".into()));

        thread::sleep_ms(1000);

        self.subscribe_to_channels(vec!("book.BTC-PERPETUAL.raw".into()));

        let command_receiver_clone = crossbeam_channel::Receiver::clone(&self.command_receiver);

        thread::scope(|s| {
            s.spawn(|| {
                let mut command_iter = command_receiver_clone.iter();

                loop {
                    match command_iter.next().unwrap() {
                        Command::MakeOrder { request_id, direction, instrument, price, amount, label } => {
                            match direction {
                                OrderSide::Ask => {
                                    self.place_order(request_id, instrument.clone(), TradeDirection::Ask, price.clone(), amount.clone(), label.clone())
                                }
                                OrderSide::Bid => {
                                    self.place_order(request_id, instrument.clone(), TradeDirection::Bid, price.clone(), amount.clone(), label.clone())
                                }
                            }.expect("TODO: panic message");
                        }

                        Command::SendHeartBeat => self.heartbeat().expect("TODO: panic message"),

                        other => warn!("Unsupported command {:?}", other),
                    };
                }
            });


            loop {
                // println!("Get socket lock on read");
                thread::sleep(Duration::from_micros(1));
                match self.socket.lock().unwrap().read_message() {
                    Ok(msg) => {
                        match msg {
                            Message::Text(s) => {
                                // println!("Got {}", s);

                                let parsed_response: Response = serde_json::from_str(&s).unwrap();

                                match parsed_response {
                                    Response::Notification { jsonrpc, method, params } =>
                                        {
                                            match method.as_str() {
                                                "subscription" => {
                                                    let channel = params["channel"].as_str().unwrap();
                                                    let data: serde_json::Value = params["data"].clone();

                                                    match channel {
                                                        x if x.starts_with("user.orders") => {
                                                            let deribit_order: Order = serde_json::from_value(data).unwrap();
                                                            let direction = match deribit_order.direction {
                                                                Direction::Buy => order_manager::TradeDirection::Bid,
                                                                Direction::Sell => order_manager::TradeDirection::Ask
                                                            };

                                                            let order_status = match deribit_order.order_state {
                                                                OrderState::Open => OrderStatus::Open,
                                                                OrderState::Filled => OrderStatus::Filled,
                                                                OrderState::Rejected => OrderStatus::Rejected,
                                                                OrderState::Cancelled => OrderStatus::Cancelled,
                                                                OrderState::Untriggered => OrderStatus::Untriggered,
                                                            };

                                                            let order = order_manager::OrderEvent::OrderChanged {
                                                                id: deribit_order.order_id,
                                                                direction,
                                                                price: deribit_order.price,
                                                                amount: deribit_order.amount, // todo or filled_amount ?
                                                                status: order_status,
                                                                label: deribit_order.label,
                                                            };

                                                            self.order_sender.send(order).unwrap();
                                                        }
                                                        x if x.starts_with("book") => {
                                                            let change = OrderbookChange::new(data);
                                                            let update = OrderbookUpdate {
                                                                timestamp: change.timestamp,
                                                                instrument_name: change.instrument_name,
                                                                change_id: change.change_id,
                                                                bids: change.bids.iter().map(|bid| PriceLevelChange{
                                                                    action: match bid.action {
                                                                        Action::New    => PriceLevelAction::New,
                                                                        Action::Change => PriceLevelAction::Change,
                                                                        Action::Delete => PriceLevelAction::Delete
                                                                    },
                                                                    price: bid.price,
                                                                    amount: bid.amount
                                                                }).collect(),
                                                                asks: change.asks.iter().map(|ask| PriceLevelChange{
                                                                    action: match ask.action {
                                                                        Action::New    => PriceLevelAction::New,
                                                                        Action::Change => PriceLevelAction::Change,
                                                                        Action::Delete => PriceLevelAction::Delete
                                                                    },
                                                                    price: ask.price,
                                                                    amount: ask.amount
                                                                }).collect(),
                                                            };

                                                            self.orderbook_sender.send(update).unwrap();
                                                        }
                                                        x if x.starts_with("user.portfolio") => {
                                                            let portfolio_update: Portfolio = serde_json::from_value(data).unwrap();
                                                            let balance = Balance {balance: portfolio_update.balance};
                                                            self.portfolio_sender.send(balance).unwrap();
                                                        }

                                                        x if x.starts_with("trades.") => {
                                                            let trade: Trade = serde_json::from_value(data).unwrap();
                                                        }
                                                        x => warn!("Unexpected channel {}", x)
                                                    }
                                                }
                                                "heartbeat" => {
                                                    self.command_sender.send(Command::SendHeartBeat);
                                                    info!("Got heartbeat")
                                                }
                                                otherwise => warn!("Got smth else in Notification {}", otherwise)
                                            }
                                        }
                                    Response::Result { jsonrpc, id, result, us_in, us_out, us_diff, testnet } =>
                                        {
                                            info!("Got Response::Result {}, id {}", result, id);

                                            let response = order_manager::OrderEvent::OrderSuccess { uuid: id };
                                            self.order_sender.send(response);

                                            // match result.as_str() {
                                            //     Some(x) if x.starts_with("user.orders") => {
                                            //         let response = order_manager::OrderEvent::OrderSuccess { uuid: id };
                                            //         self.order_sender.send(response);
                                            //     }
                                            //     _ => ()
                                            // };
                                            //
                                            // let order:Result<Order, dyn Error> = serde_json::from_value(result);
                                            // match order {
                                            //     Ok(_) => {
                                            //         let response = order_manager::OrderEvent::OrderSuccess { uuid: id };
                                            //         self.order_sender.send(response);
                                            //     }
                                            //     _ => ()
                                            // };
                                        }
                                    Response::Error { jsonrpc, id, error, us_in, us_out, us_diff, testnet } =>
                                        {
                                            error!("Got Response::Error {}, id {}", error.message, id);
                                            // if result.starts_with("user.orders") {
                                            let response = order_manager::OrderEvent::OrderSuccess { uuid: id };
                                            self.order_sender.send(response);
                                            // }
                                        }
                                }
                            }
                            Message::Close(_) => {
                                warn!("Got Close frame. Reconnect");
                                self.reconnect();
                            }
                            _ => {
                                warn!("Got unexpected {:?}", msg);
                            }
                        };
                    }
                    Err(e) => {
                        error!("Got error on reading from socket{:?}", e);
                        self.reconnect();
                    }
                };
                // println!("Release socket lock on read");
            }
        });
    }

    pub fn new(orderbook_sender: Sender<OrderbookUpdate>,
               order_sender: Sender<order_manager::OrderEvent>,
               raw_data_sender: Sender<String>,
               command_receiver: Receiver<Command>,
               portfolio_sender: Sender<Balance>,
               command_sender: Sender<Command>,
    ) -> DeribitConnector {
        let (socket1, response) = connect(
            Url::parse("wss://test.deribit.com/ws/api/v2").unwrap()
        ).expect("Can't connect");

        let socket = Arc::new(Mutex::new(socket1));

        DeribitConnector { orderbook_sender, order_sender, raw_data_sender, command_receiver, socket, portfolio_sender, command_sender }
    }

    fn reconnect(&self) {
        warn!("Trying to reconnect....");

        let (socket, response) = connect(
            Url::parse("wss://test.deribit.com/ws/api/v2").unwrap()
        ).expect("Can't connect");

        let mut mutex = self.socket.lock().unwrap();
        *mutex = socket;

        self.set_heartbeat_interval(60);
    }

    // fn create_socket(&self) -> WebSocket<MaybeTlsStream<TcpStream>> {
    //     match connect(Url::parse("wss://test.deribit.com/ws/api/v2").unwrap()) {
    //         Ok((socket, response)) => socket
    //     }
    // }

    fn subscribe_to_channels(&self, channels: Vec<String>) -> Result<(), Box<dyn Error>> {
        let to_subscribe = Params::Channels { channels };

        let subscribe_request = JsonRpcRequest::new("public/subscribe".to_string(), Uuid::new_v4(), Some(to_subscribe));

        info!("Sending channel subscribing request {:?}", subscribe_request);

        self.send_request(subscribe_request)
    }

    fn subscribe_to_orders(&self, channels: Vec<String>) -> Result<(), Box<dyn Error>> {
        let to_subscribe = Params::Channels { channels };

        let subscribe_request = JsonRpcRequest::new("private/subscribe".to_string(), Uuid::new_v4(), Some(to_subscribe));

        info!("Sending order subscribing request {:?}", subscribe_request);

        self.send_request(subscribe_request)
    }

    fn subscribe_to_portfolio_channel(&self, channels: Vec<String>) -> Result<(), Box<dyn Error>> {
        let to_subscribe = Params::Channels { channels };

        let subscribe_request = JsonRpcRequest::new("private/subscribe".to_string(), Uuid::new_v4(), Some(to_subscribe));

        info!("Sending portfolio subscribing request {:?}", subscribe_request);

        self.send_request(subscribe_request)
    }

    fn set_heartbeat_interval(&self, interval: u32) -> Result<(), Box<dyn Error>> {
        let heartbeat_interval = Params::Interval { interval };

        let set_heartbeat_request = JsonRpcRequest::new("public/set_heartbeat".to_string(), Uuid::new_v4(), Some(heartbeat_interval));

        info!("Sending heartbeat interval {:?}", set_heartbeat_request);

        self.send_request(set_heartbeat_request)
    }

    fn heartbeat(&self) -> Result<(), Box<dyn Error>> {
        let heartbeat = JsonRpcRequest::new("public/test".to_string(), Uuid::new_v4(), None);

        info!("Sending heartbeat {:?}", heartbeat);

        self.send_request(heartbeat)
    }

    fn authorize(&self) -> Result<(), Box<dyn Error>> {
        let auth = Params::Auth {
            grant_type: "client_credentials".to_string(),
            client_id: env!("client_id").to_string(),
            client_secret: env!("client_secret").to_string(),
        };


        let auth_request = JsonRpcRequest::new("public/auth".to_string(), Uuid::new_v4(), Some(auth));

        info!("Sending auth request {:?}", auth_request);

        self.send_request(auth_request)
    }

    fn place_order(&self, request_id: Uuid, instrument: String, direction: TradeDirection, price: Decimal, amount: Decimal, label: String) -> Result<(), Box<dyn Error>> {
        let method = match direction {
            TradeDirection::Ask => "private/sell",
            TradeDirection::Bid => "private/buy"
        };

        let order = Params::Order {
            instrument_name: instrument,
            price,
            amount,
            post_only: true,
            label,
        };

        let request = JsonRpcRequest::new(method.to_string(), request_id, Some(order));

        // info!("Sending order making request {:?}", request);

        self.send_request(request)
    }

    fn make_order(&self, instrument: String, direction: TradeDirection, price: Decimal, amount: Decimal, label: String) -> JsonRpcRequest {
        let method = match direction {
            TradeDirection::Ask => "private/sell",
            TradeDirection::Bid => "private/buy"
        };

        let order = Params::Order {
            instrument_name: instrument,
            price,
            amount,
            post_only: true,
            label,
        };

        JsonRpcRequest::new(method.to_string(), Uuid::new_v4(), Some(order))
    }


    fn send_request(&self, request: JsonRpcRequest) -> Result<(), Box<dyn Error>> {
        let s = serde_json::to_string(&request)?;

        info!("Sending request: {:?}", s);
        // println!("lock write");


        // let mut lock = self.socket.try_lock();
        //
        // if let Ok(ref mut mutex) = lock {
        //     mutex.write_message(Message::Text(s)).unwrap();
        //     info!("Sent request");
        // }  else {
        //     println!("try_lock failed");
        // };
        //
        // Ok(())


        // println!("Get socket lock on write");
        let r = match self.socket.lock().unwrap().write_message(Message::Text(s)) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::try_from(format!("Error {:?}", e)).unwrap()),
        };
        // println!("Release socket lock on write");
        r
    }


    fn read_command_channel(&self) {}
}


// 2023-02-07T15:16:40.820471+02:00 WARN ct::ws_connector::deribit_ws - Trying to reconnect....
// thread '<unnamed>' panicked at 'Can't connect: Http(Response { status: 502, version: HTTP/1.1, headers: {"server": "nginx/1.21.3", "date": "Tue, 07 Feb 2023 13:16:41 GMT", "content-type": "text/html", "content-length": "20783", "connection": "keep-alive", "etag": "\"63e11330-512f\""}, body: None })', src/ws_connector.rs:460:15
// stack backtrace:
// 0: rust_begin_unwind
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/panicking.rs:584:5
// 1: core::panicking::panic_fmt
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/panicking.rs:142:14
// 2: core::result::unwrap_failed
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/result.rs:1814:5
// 3: core::result::Result<T,E>::expect
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/result.rs:1064:23
// 4: ct::ws_connector::deribit_ws::DeribitConnector::reconnect
// at ./src/ws_connector.rs:458:38
// 5: ct::ws_connector::deribit_ws::DeribitConnector::run::{{closure}}
// at ./src/ws_connector.rs:431:29
// 6: std::thread::scoped::scope::{{closure}}
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/thread/scoped.rs:146:51
// 7: <core::panic::unwind_safe::AssertUnwindSafe<F> as core::ops::function::FnOnce<()>>::call_once
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/panic/unwind_safe.rs:271:9
// 8: std::panicking::try::do_call
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/panicking.rs:492:40
// 9: ___rust_try
// 10: std::panicking::try
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/panicking.rs:456:19
// 11: std::panic::catch_unwind
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/panic.rs:137:14
// 12: std::thread::scoped::scope
// at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/thread/scoped.rs:146:18
// 13: ct::ws_connector::deribit_ws::DeribitConnector::run
// at ./src/ws_connector.rs:302:13
// 14: ct::main::{{closure}}
// at ./src/main.rs:36:9
// note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
//
// Process finished with exit code 130 (interrupted by signal 2: SIGINT)