pub mod deribit_ws {
    use std::net::TcpStream;
    use url::Url;
    use tungstenite::{connect, Message, WebSocket};
    use crossbeam_channel::{bounded, Sender, Receiver};
    use serde::{Serialize, Deserialize};
    use serde_json::Value;
    use tungstenite::stream::MaybeTlsStream;
    use std::error::Error;
    use rust_decimal::Decimal;

    use log::{error, info, warn};
    use uuid::Uuid;
    use crate::strategy::order_manager;

    #[derive(Serialize, Deserialize)]
    pub(crate) struct RpcError {
        message: String,
        code: i32,
        data: Option<Value>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    #[derive(Debug)]
    pub(crate) enum Params {
        Channels { channels: Vec<String> },
        Interval { interval: u32 },
        Auth { grant_type: String, client_id: String, client_secret: String },
        Buy { instrument_name: String, price: Decimal, amount: Decimal, post_only: bool },
        Sell { instrument_name: String, price: Decimal, amount: Decimal, post_only: bool },
    }

    #[derive(Serialize, Deserialize)]
    #[derive(Debug)]
    pub(crate) struct JsonRpcRequest {
        jsonrpc: String,
        method: String,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Params>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    pub(crate) enum Response {
        Notification {
            jsonrpc: String,
            method: String,
            params: Value,
        },
        #[serde(rename_all = "camelCase")]
        Error {
            jsonrpc: String,
            id: Uuid,
            error: RpcError,
            us_in: u64,
            us_out: u64,
            us_diff: u32,
            testnet: bool,
        },
        #[serde(rename_all = "camelCase")]
        Result {
            jsonrpc: String,
            id: Uuid,
            result: Value,
            us_in: u64,
            us_out: u64,
            us_diff: u32,
            testnet: bool,
        },
    }


    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[derive(Debug)]
    #[derive(PartialEq)]
    pub enum PriceAction {
        New,
        Change,
        Delete
    }

    #[derive(Serialize, Deserialize)]
    #[derive(PartialEq)]
    #[derive(Debug)]
    pub struct PriceLevel {
        pub(crate) action: PriceAction,
        pub(crate) price: Decimal,
        pub(crate) amount: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[derive(Debug)]
    pub struct OrderbookUpdate {
        pub timestamp: i64,
        pub instrument_name: String,
        pub change_id: i64,
        pub bids: Vec<PriceLevel>,
        pub asks: Vec<PriceLevel>,
    }

    impl OrderbookUpdate {
        pub fn new(s: Value) -> OrderbookUpdate {
            let timestamp = s["timestamp"].clone().as_i64().unwrap();
            let change_id = s["change_id"].clone().as_i64().unwrap();
            let instrument_name = s["instrument_name"].clone().as_str().unwrap().to_string();


            let l = s["bids"].as_array().unwrap().clone();

            let bids = l.into_iter().map(|q| serde_json::from_value(q).unwrap()).collect();
            let asks = s["asks"].as_array().unwrap().clone().into_iter().map(|q| serde_json::from_value(q).unwrap()).collect();


            OrderbookUpdate {
                timestamp,
                instrument_name,
                change_id,
                bids,
                asks,
            }
        }
    }

    // todo union with order_manager
    #[derive(Debug)]
    pub enum TradeDirection {
        Bid,
        Ask,
    }

    #[derive(Debug)]
    pub enum Command {
        SubscribeData { channel: String },
        UnsubscribeData { channel: String },
        MakeOrder { direction: TradeDirection, instrument: String, price: Decimal, amount: Decimal },
        CancelOrder { id: String },
        CancelAll,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[derive(Debug)]
    pub enum OrderState {
        Open,
        Filled,
        Rejected,
        Cancelled,
        Untriggered,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    #[derive(Debug)]
    pub enum TimeInForce {
        GoodTilCancelled,
        GoodTilDay,
        FillOrKill,
        ImmediateOrCancel,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    #[derive(Debug)]
    pub enum Direction {
        Buy,
        Sell,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    #[derive(Debug)]
    pub enum OrderType {
        Limit,
        Market,
        StopLimit,
        StopMarket,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    #[derive(Debug)]
    pub enum Trigger {
        IndexPrice,
        MarkPrice,
        LastPrice,
    }

    #[derive(Serialize, Deserialize)]
    #[derive(Debug)]
    pub struct Order {
        mmp_cancelled: Option<bool>,
        order_state: OrderState,
        max_show: Decimal,
        reject_post_only: bool,
        api: bool,
        amount: Decimal,
        web: Option<bool>,
        instrument_name: String,
        advanced: Option<String>,
        triggered: Option<bool>,
        block_trade: Option<bool>,
        original_order_type: Option<String>,
        trigger_offset: Option<Decimal>,
        price: Decimal,
        time_in_force: TimeInForce,
        auto_replaced: Option<bool>,
        last_update_timestamp: i64,
        post_only: bool,
        replaced: bool,
        filled_amount: Decimal,
        average_price: Decimal,
        order_id: String,
        reduce_only: bool,
        commission: Decimal,
        app_name: Option<String>,
        label: String,
        trigger_order_id: Option<String>,
        trigger_price: Option<Decimal>,
        creation_timestamp: i64,
        direction: Direction,
        is_liquidation: bool,
        order_type: OrderType,
        usd: Option<Decimal>,
        profit_loss: Decimal,
        trigger_reference_price: Option<Decimal>,
        risk_reducing: bool,
        implv: Option<Decimal>,
        trigger: Option<Trigger>,
    }


    #[derive(Serialize, Deserialize)]
    #[derive(Debug)]
    pub struct Portfolio {
        available_funds: Decimal,
        available_withdrawal_funds: Decimal,
        pub balance: Decimal,
        currency: String,
        delta_total: Decimal,
        delta_total_map: Value,
        equity: Decimal,
        estimated_liquidation_ratio: Option<Decimal>,
        estimated_liquidation_ratio_map: Option<Value>,
        fee_balance: Decimal,
        futures_pl: Decimal,
        futures_session_rpl: Decimal,
        futures_session_upl: Decimal,
        initial_margin: Decimal,
        maintenance_margin: Decimal,
        margin_balance: Decimal,
        options_delta: Decimal,
        options_gamma: Decimal,
        options_pl: Decimal,
        options_session_rpl: Decimal,
        options_session_upl: Decimal,
        options_theta: Decimal,
        options_value: Decimal,
        options_vega: Decimal,
        portfolio_margining_enabled: bool,
        projected_delta_total: Decimal,
        projected_initial_margin: Decimal,
        projected_maintenance_margin: Decimal,
        session_rpl: Decimal,
        session_upl: Decimal,
        total_pl: Decimal,
    }


    pub struct DeribitConnector {
        orderbook_sender: Sender<OrderbookUpdate>,
        order_sender: Sender<order_manager::Order>,
        raw_data_sender: Sender<String>,
        command_receiver: Receiver<Command>,
        socket: WebSocket<MaybeTlsStream<TcpStream>>,
    }

    impl DeribitConnector {
        pub fn run(&mut self) {
            self.connect();

            self.authorize();

            self.subscribe_to_channels(vec!("book.BTC-PERPETUAL.raw".into()));
            self.subscribe_to_orders(vec!("user.orders.BTC-PERPETUAL.raw".into()));

            loop {
                match self.socket.read_message() {
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
                                                            // self.orderbook_sender.send(update).unwrap(); // blocking !
                                                            // self.raw_data_sender.send(s.clone()).unwrap();

                                                            let direction = match deribit_order.direction {
                                                                Direction::Buy => order_manager::TradeDirection::Bid,
                                                                Direction::Sell => order_manager::TradeDirection::Ask
                                                            };


                                                            let order = order_manager::Order{
                                                                id: deribit_order.order_id,
                                                                direction,
                                                                price: deribit_order.price,
                                                                amount: deribit_order.amount, // todo or filled_amount ?
                                                                status: deribit_order.order_state
                                                            };

                                                            self.order_sender.send(order).unwrap();
                                                        }
                                                        x if x.starts_with("book") => {
                                                            let update = OrderbookUpdate::new(data);
                                                            // self.orderbook_sender.send(update).unwrap(); // blocking !
                                                            // self.raw_data_sender.send(s.clone()).unwrap();
                                                            // self.orderbook_sender.send(update).unwrap();
                                                        }
                                                        x => warn!("Unexpected channel {}", x)
                                                    }
                                                }
                                                "heartbeat" => {
                                                    self.heartbeat();
                                                    info!("Got heartbeat")
                                                }
                                                otherwise => warn!("Got smth else in Notification {}", otherwise)
                                            }
                                        }
                                    Response::Result { jsonrpc, id, result, us_in, us_out, us_diff, testnet } =>
                                        {
                                            info!("Got Response::Result {}, id {}", result, id);
                                        }
                                    Response::Error { jsonrpc, id, error, us_in, us_out, us_diff, testnet } =>
                                        {
                                            error!("Got Response::Error {}, id {}", error.message, id);
                                        }
                                }
                            }
                            Message::Close(_) => {
                                warn!("Got Close frame. Reconnect");
                                self.connect();
                                continue;
                            }
                            _ => {
                                warn!("Got unexpected {:?}", msg);
                                continue;
                            }
                        };
                    }
                    Err(e) => {
                        error!("Got error on reading from socket{:?}", e);
                        self.connect();
                    }
                };
            }
        }

        pub fn new(orderbook_sender: Sender<OrderbookUpdate>,
                   order_sender: Sender<order_manager::Order>,
                   raw_data_sender: Sender<String>,
                   command_receiver: Receiver<Command>) -> DeribitConnector {
            let (socket, response) = connect(
                Url::parse("wss://test.deribit.com/ws/api/v2").unwrap()
            ).expect("Can't connect");

            DeribitConnector { orderbook_sender, order_sender, raw_data_sender, command_receiver, socket }
        }

        fn connect(&mut self) {
            let (socket, responce) = connect(
                Url::parse("wss://test.deribit.com/ws/api/v2").unwrap()
            ).expect("Can't connect");

            self.socket = socket;

            self.set_heartbeat_interval(60);
        }

        fn subscribe_to_channels(&mut self, channels: Vec<String>) -> Result<(), Box<dyn Error>> {
            let to_subscribe = Params::Channels { channels };

            let subscribe_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "public/subscribe".to_string(),
                id: Uuid::new_v4(),
                params: Some(to_subscribe),
            };

            info!("Sending channel subscribing request {:?}", subscribe_request);

            self.send_request(subscribe_request)
        }

        fn subscribe_to_orders(&mut self, channels: Vec<String>) -> Result<(), Box<dyn Error>> {
            let to_subscribe = Params::Channels { channels };

            let subscribe_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "private/subscribe".to_string(),
                id: Uuid::new_v4(),
                params: Some(to_subscribe),
            };

            info!("Sending order subscribing request {:?}", subscribe_request);

            self.send_request(subscribe_request)
        }

        fn set_heartbeat_interval(&mut self, interval: u32) -> Result<(), Box<dyn Error>> {


            let heartbeat_interval = Params::Interval { interval };

            let set_heartbeat_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "public/set_heartbeat".to_string(),
                id: Uuid::new_v4(),
                params: Some(heartbeat_interval),
            };

            info!("Sending heartbeat interval {:?}", set_heartbeat_request);

            self.send_request(set_heartbeat_request)
        }

        fn heartbeat(&mut self) -> Result<(), Box<dyn Error>> {

            let heartbeat = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "public/test".to_string(),
                id: Uuid::new_v4(),
                params: None,
            };

            info!("Sending heartbeat {:?}", heartbeat);

            self.send_request(heartbeat)
        }

        fn authorize(&mut self) -> Result<(), Box<dyn Error>> {
            let auth = Params::Auth {
                grant_type: "client_credentials".to_string(),
                client_id: "g7gyJMiN".to_string(),
                client_secret: "RMBbHnBZTZN_Oala2AVVTLEMSQRtbHGnSNmjpChfvVs".to_string(),
            };


            let auth_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "public/auth".to_string(),
                id: Uuid::new_v4(),
                params: Some(auth),
            };

            info!("Sending auth request {:?}", auth_request);

            self.send_request(auth_request)
        }

        fn mk_order(&mut self, instrument: String, direction: TradeDirection, price: Decimal, amount: Decimal) -> Result<(), Box<dyn Error>> {
            let order = match direction {
                TradeDirection::Ask => Params::Sell {
                    instrument_name: instrument,
                    price,
                    amount,
                    post_only: true,
                },
                TradeDirection::Bid => Params::Buy {
                    instrument_name: instrument,
                    price,
                    amount,
                    post_only: true,
                }
            };

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "public/auth".to_string(),
                id: Uuid::new_v4(),
                params: Some(order),
            };

            info!("Sending order making request {:?}", request);

            self.send_request(request)
        }


        fn send_request(&mut self, request: JsonRpcRequest) -> Result<(), Box<dyn Error>> {
            let s = serde_json::to_string(&request)?;

            match self.socket.write_message(Message::Text(s)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::try_from(format!("Error {:?}", e)).unwrap()),
            }
        }


        fn read_command_channel(&mut self) {
            // let mut command_iter = self.command_receiver.iter();
            //
            //
            // loop {
            //     match command_iter.next().unwrap() {
            //         Command::MakeOrder { direction, instrument, price, amount } => {
            //             match direction {
            //                 TradeDirection::Ask =>
            //                     self.mk_order(101, instrument.clone(), TradeDirection::Ask, price.clone(), amount.clone()),
            //                 TradeDirection::Bid =>
            //                     self.mk_order(101, instrument.clone(), TradeDirection::Bid, price.clone(), amount.clone()),
            //             }.expect("TODO: panic message");
            //         }
            //
            //         other => warn!("Unsupported command {:?}", other),
            //     };
            // }
        }
    }


    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn check_json_deserialize() {
            let orderbook_update_notification = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"book.ETH-PERPETUAL.100.1.100ms","data":{"timestamp":1662760941557,"instrument_name":"ETH-PERPETUAL","change_id":2770450294,"bids":[[1898.0,1150.0]],"asks":[[2222.0,333.0]]}}}"#;

            let heartbeat_notification = r#"{"params":{"type":"test_request"},"method":"heartbeat","jsonrpc":"2.0"}"#;

            let subscribe_result = r#"{"jsonrpc":"2.0","id":"b1288e7d-5f00-4d7f-b89f-66ae19b56563","result":["book.ETH-PERPETUAL.100.1.100ms"],"usIn":1662838375635617,"usOut":1662838375635666,"usDiff":49,"testnet":true}"#;

            let heartbeat_result = r#"{"jsonrpc":"2.0","id":"b1288e7d-5f00-4d7f-b89f-66ae19b56563","result":"ok","usIn":1662838375635549,"usOut":1662838375635580,"usDiff":31,"testnet":true}"#;

            let error = r#"{"jsonrpc":"2.0","error":{"message":"bad_request","code":11050},"usIn":1663413774140402,"usOut":1663413774140419,"usDiff":17,"testnet":true}"#;

            let v: Response = serde_json::from_str(orderbook_update_notification).unwrap();

            match v {
                Response::Notification { jsonrpc, method, params } => {
                    assert_eq!(jsonrpc, "2.0");
                    assert_eq!(method, "subscription");
                    assert_eq!(params["channel"], "book.ETH-PERPETUAL.100.1.100ms");
                    assert_eq!(params["data"]["timestamp"], 1662760941557_i64);
                }
                other => panic!("Unexpected parsing result"),
            };

            let e: Response = serde_json::from_str(subscribe_result).unwrap();

            match e {
                Response::Result { jsonrpc, id, result, us_in, us_out, us_diff, testnet } => {
                    assert_eq!(jsonrpc, "2.0");
                    assert_eq!(id, Uuid::parse_str("b1288e7d-5f00-4d7f-b89f-66ae19b56563").unwrap());
                }
                other => panic!("Unexpected parsing result"),
            }
        }

        #[test]
        fn check_orderbook_update_deserialize() {
            let orderbook_update_notification = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"book.ETH-PERPETUAL.100.1.100ms","data":{"timestamp":1662760941557,"instrument_name":"ETH-PERPETUAL","change_id":2770450294,"bids":[["new", 1898.0,1150.0], [2222.0,333.0]],"asks":[["new", 2222.0,333.0]]}}}"#;

            let v: Response = serde_json::from_str(orderbook_update_notification).unwrap();

            match v {
                Response::Notification { jsonrpc, method, params } => {
                    let data: serde_json::Value = params["data"].clone();

                    let update = OrderbookUpdate::new(data);

                    assert_eq!(update.bids, vec!(PriceLevel { action: PriceAction::New, price: Decimal::from_f64_retain(1898.0).unwrap(), amount: Decimal::from_f64_retain(1150.0).unwrap() }, PriceLevel {action: PriceAction::New, price: Decimal::from_f64_retain(2222.0).unwrap(), amount: Decimal::from_f64_retain(333.0).unwrap() }))
                }
                other => panic!("Unexpected parsing result"),
            };
        }

        #[test]
        fn check_order_deserialize() {
            let order = r#"{"jsonrpc":"2.0","method":"subscription","params":{"channel":"user.orders.BTC-PERPETUAL.raw","data":{"web":true,"time_in_force":"good_til_cancelled","risk_reducing":false,"replaced":false,"reject_post_only":false,"reduce_only":false,"profit_loss":0.0,"price":19094.0,"post_only":true,"order_type":"limit","order_state":"open","order_id":"14490265484","mmp":false,"max_show":10.0,"last_update_timestamp":1665867451646,"label":"","is_liquidation":false,"instrument_name":"BTC-PERPETUAL","filled_amount":0.0,"direction":"buy","creation_timestamp":1665867451646,"commission":0.0,"average_price":0.0,"api":false,"amount":10.0}}}"#;

            let v: Response = serde_json::from_str(order).unwrap();

            match v {
                Response::Notification { jsonrpc, method, params } => {
                    let data: serde_json::Value = params["data"].clone();

                    let update: Order = serde_json::from_value(data).unwrap();

                    // assert_eq!(update.bids, vec!(PriceLevel { price: Decimal::from_f64_retain(1898.0).unwrap(), amount: Decimal::from_f64_retain(1150.0).unwrap() }, PriceLevel { price: Decimal::from_f64_retain(2222.0).unwrap(), amount: Decimal::from_f64_retain(333.0).unwrap() }))
                }
                other => panic!("Unexpected parsing result"),
            };
        }

        #[test]
        fn check_subscription_request_serialize() {
            let expected = r#"{"jsonrpc": "2.0",
            "method": "public/subscribe",
            "id": 42,
            "params": {"channels": ["book.ETH-PERPETUAL.100.1.100ms"]}}"#;


            let expected2 = r#"{"jsonrpc" : "2.0",
                "id" : 42,
                "method" : "public/set_heartbeat", "params" : {"interval" : 60}}"#;
        }
    }
}