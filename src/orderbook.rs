#![feature(map_first_last)]

use std::collections::btree_map::BTreeMap;
use std::ops::Index;
use rust_decimal::Decimal;
use crate::core::entities::PriceLevelChange;
use crate::core::entities::PriceLevelAction;

// bid - buys
// ask - sells
pub struct TreeOrderBook {
    asks: BTreeMap<Decimal, Decimal>,
    bids: BTreeMap<Decimal, Decimal>
}


impl TreeOrderBook {

    pub fn new() -> TreeOrderBook {
        TreeOrderBook{asks: BTreeMap::new(), bids: BTreeMap::new()}
    }

    pub fn add_ask(&mut self, price: Decimal, quantity: Decimal) {
        self.asks.insert(price, quantity);
    }

    pub fn change_ask(&mut self, price: Decimal, quantity: Decimal) {
        // if self.asks.contains_key(price.into()) {
            let current = self.asks.entry(price).or_insert(Decimal::from(0));
            *current = quantity;
        // }
    }

    pub fn best_ask(&self) -> Option<(&Decimal, &Decimal)> {
        self.asks.iter().next()
    }

    pub fn delete_ask(&mut self, price: Decimal) {
        self.asks.remove(&price);
    }


    pub fn add_asks(&mut self, asks: Vec<PriceLevelChange>) {
        for price_level in asks {
            match price_level.action {
                PriceLevelAction::New => self.add_ask(price_level.price, price_level.amount),
                PriceLevelAction::Change => self.change_ask(price_level.price, price_level.amount),
                PriceLevelAction::Delete => self.delete_ask(price_level.price),
            }
        }
        // println!("asks: {:?}", self.asks)
    }

    pub fn get_nth_ask(&self, n: usize) -> Option<(&Decimal, &Decimal)> {
        self.asks.iter().skip(n).next()
    }

    pub fn add_bid(&mut self, price: Decimal, quantity: Decimal) {
        self.bids.insert(price, quantity);
    }

    pub fn add_bids(&mut self, bids: Vec<PriceLevelChange>) {
        for price_level in bids {
            match price_level.action {
                PriceLevelAction::New => self.add_bid(price_level.price, price_level.amount),
                PriceLevelAction::Change => self.change_bid(price_level.price, price_level.amount),
                PriceLevelAction::Delete => self.delete_bid(price_level.price),
            }
        }
        // println!("bids: {:?}", self.bids)
    }

    pub fn change_bid(&mut self, price: Decimal, quantity: Decimal) {
        // if self.asks.contains_key(price.into()) {
        let current = self.bids.entry(price).or_insert(Decimal::from(0));
        *current = quantity;
        // }
    }

    pub fn delete_bid(&mut self, price: Decimal) {
        self.bids.remove(&Decimal::from(price));
    }

    pub fn best_bid(&self) -> Option<(&Decimal, &Decimal)> {
        self.bids.iter().next_back()
    }

    pub fn get_nth_bid(&self, n: usize) -> Option<(&Decimal, &Decimal)> {
        self.bids.iter().skip(n).next_back()
    }

    pub fn get_spread(&self) -> Option<Decimal> {
        let bid = self.best_bid();
        let ask = self.best_ask();

        bid.and_then(|b| ask.map(|a| a.0 - b.0))
    }
}


#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use crate::orderbook::TreeOrderBook;

    #[test]
    fn check_ask_insert() {
        // assert_eq!
        // assert_ne!
        // #[should_panic(expected = )]

        let mut orderbook = TreeOrderBook::new();

        orderbook.add_ask(Decimal::from(100), Decimal::from(100));
        orderbook.add_ask(Decimal::from(200), Decimal::from(100));
        orderbook.add_ask(Decimal::from(300), Decimal::from(100));

        assert_eq!(orderbook.best_ask(), Some((&Decimal::from(100), &Decimal::from(100))));
    }

    #[test]
    fn check_bid_insert() {
        let mut orderbook = TreeOrderBook::new();

        orderbook.add_bid(Decimal::from(100), Decimal::from(100));
        orderbook.add_bid(Decimal::from(200), Decimal::from(100));
        orderbook.add_bid(Decimal::from(300), Decimal::from(100));

        assert_eq!(orderbook.best_bid(), Some((&Decimal::from(300), &Decimal::from(100))));
    }


    #[test]
    fn check_spread() {
        let mut orderbook = TreeOrderBook::new();

        orderbook.add_bid(Decimal::from(100), Decimal::from(100));
        orderbook.add_bid(Decimal::from(200), Decimal::from(100));
        orderbook.add_bid(Decimal::from(300), Decimal::from(100));

        orderbook.add_ask(Decimal::from(400), Decimal::from(100));
        orderbook.add_ask(Decimal::from(500), Decimal::from(100));
        orderbook.add_ask(Decimal::from(600), Decimal::from(100));

        assert_eq!(orderbook.get_spread(), Some(Decimal::from(100)));
    }

}