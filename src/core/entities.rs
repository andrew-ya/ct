use serde::{Serialize, Deserialize};
use serde_json::Value;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
pub enum OrderSide {
    Ask, Bid
}

#[derive(Debug)]
pub enum Command {
    SubscribeData { channel: String },
    UnsubscribeData { channel: String },
    MakeOrder { request_id: Uuid, direction: OrderSide, instrument: String, price: Decimal, amount: Decimal, label: String },
    CancelOrder { id: String },
    CancelAll,
    SendHeartBeat,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
#[derive(PartialEq)]
pub enum PriceLevelAction {
    New,
    Change,
    Delete,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct PriceLevelChange {
    pub(crate) action: PriceLevelAction,
    pub(crate) price: Decimal,
    pub(crate) amount: Decimal,
}


#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct
    OrderbookUpdate {
        pub(crate) timestamp: i64,
        pub(crate) instrument_name: String,
        pub(crate) change_id: i64,
        pub(crate) bids: Vec<PriceLevelChange>,
        pub(crate) asks: Vec<PriceLevelChange>,
    }
