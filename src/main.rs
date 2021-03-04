#![allow(unused)]


use std::str::FromStr;


fn main() {
    match websocat_api::StrNode::from_str(&std::env::args().nth(1).unwrap()) {
        Ok(x) => println!("{}", x),
        Err(e) => println!("{:#}", e),
    }
}

