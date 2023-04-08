use serde::{Serialize, Deserialize};
use serde_json::Value;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub enum TradeDirection {
    Bid,
    Ask,
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

#[derive(Debug)]
#[derive(PartialEq)]
pub enum PriceLevelAction {
    New,
    Change,
    Delete,
}

#[derive(Debug)]
pub struct PriceLevelChange {
    pub(crate) action: PriceLevelAction,
    pub(crate) price: Decimal,
    pub(crate) amount: Decimal,
}


#[derive(Debug)]
pub struct OrderbookUpdate {
    pub(crate) timestamp: i64,
    pub(crate) instrument_name: String,
    pub(crate) change_id: i64,
    pub(crate) bids: Vec<PriceLevelChange>,
    pub(crate) asks: Vec<PriceLevelChange>,
}


#[derive(Debug)]
pub struct Trade {
    amount: Decimal,
    block_trade_id: Option<String>,
    direction: TradeDirection,
    index_price: Decimal,
    instrument_name: String,
    iv: Option<Decimal>,
    liquidation: Option<String>,
    mark_price: Decimal,
    price: Decimal,
    tick_direction: u8,
    timestamp: i64,
    trade_id: String,
    trade_seq: i64,
}

impl Trade {
    pub fn new(amount: Decimal,
               block_trade_id: Option<String>,
               direction: TradeDirection,
               index_price: Decimal,
               instrument_name: String,
               iv: Option<Decimal>,
               liquidation: Option<String>,
               mark_price: Decimal,
               price: Decimal,
               tick_direction: u8,
               timestamp: i64,
               trade_id: String,
               trade_seq: i64) -> Trade {
        Trade {
            amount,
            block_trade_id,
            direction,
            index_price,
            instrument_name,
            iv,
            liquidation,
            mark_price,
            price,
            tick_direction,
            timestamp,
            trade_id,
            trade_seq,
        }
    }
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
pub struct OrderChanged {
    id: String,
    direction: TradeDirection,
    price: Decimal,
    amount: Decimal,
    status: OrderStatus,
    label: String,
}

#[derive(Debug)]
pub struct OrderSuccess {
    pub(crate) uuid: Uuid,
}

#[derive(Debug)]
pub enum OrderEvent {
    OrderChangedEvent(Box<OrderChanged>),
    OrderSuccessEvent(Box<OrderSuccess>),
}

#[derive(Debug)]
pub enum MarketEvent {
    OrderBookUpdateEvent(Box<OrderbookUpdate>),
    TradeEvent(Box<Trade>),
}


#[derive(Debug)]
pub struct Order {
    id: String,
    pub(crate) direction: TradeDirection,
    price: Decimal,
    amount: Decimal,
    status: OrderStatus,
    label: String,
}

impl Order {
    pub fn new(id: String,
               direction: TradeDirection,
               price: Decimal,
               amount: Decimal,
               status: OrderStatus,
               label: String) -> Order {
        Order {
            id,
            direction,
            price,
            amount,
            status,
            label,
        }
    }
}


pub struct Balance {
    pub(crate) balance: Decimal,
}