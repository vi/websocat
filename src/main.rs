#![allow(unused)]

mod api;

use std::str::FromStr;

use futures::FutureExt;
use async_trait::async_trait;

#[macro_use]
extern crate slab_typesafe;

fn main() {
    match api::StringyNode::from_str(&std::env::args().nth(1).unwrap()) {
        Ok(x) => println!("{}", x),
        Err(e) => println!("{:#}", e),
    }
}

