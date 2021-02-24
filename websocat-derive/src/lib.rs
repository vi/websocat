extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DataStruct, DeriveInput};

#[proc_macro_derive(MyMacroHere, attributes(qqq))]
pub fn my_macro_here_derive(input: TokenStream) -> TokenStream { 
    //proc_macro::TokenTree::Group(proc_macro::Group::new(proc_macro::Delimiter::Parenthesis, proc_macro::TokenStream::new()))
    let x = parse_macro_input!(input as DeriveInput);
    eprintln!("{:#?}", x);
    TokenStream::new()
}