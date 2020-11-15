#![allow(unused)]

mod api;

use futures::FutureExt;
use async_trait::async_trait;

fn main() {
    let q : Box<dyn Lol + Send + Sync + 'static> = Box::new(Www);
    q.qqq();

    let mut a : Option<std::pin::Pin<Box<dyn std::future::Future<Output=()> + Send + 'static>>> = None;

    a = Some(async {

    }.boxed());
}


#[async_trait]
trait Lol {
    async fn qqq(&self) -> () ;
    /*fn qqq<'a,'l>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output=()> + Send + 'l>>
        where 'a : 'l
    ;*/
}
struct Www;


#[async_trait]
impl Lol for Www {
    
    fn qqq<'a, 'l>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output=()> + Send + 'l >> where 'a : 'l{
        async {

        }.boxed()
    }

    /*
    async fn qqq(&self) {

    }
    */
}