extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(MyMacroHere, attributes(qqq))]
pub fn my_macro_here_derive(input: TokenStream) -> TokenStream { 
    //proc_macro::TokenTree::Group(proc_macro::Group::new(proc_macro::Delimiter::Parenthesis, proc_macro::TokenStream::new()))
    let x = parse_macro_input!(input as DeriveInput);
    eprintln!("{:#?}", x);
    TokenStream::new()
}

#[proc_macro_derive(WebsocatNode, attributes(name))]
pub fn derive_websocat_node(input: TokenStream) -> TokenStream {
    let x = parse_macro_input!(input as DeriveInput);

    let mut f = std::fs::File::create("/tmp/derive.txt").unwrap();
    use std::io::Write;
    writeln!(f, "{:#?}", x).unwrap();

    let name = x.ident;
    let classname = quote::format_ident!("{}Class", name);

    let mut properties : Vec<(syn::Ident, websocat_api::PropertyValueType, bool)> = Vec::new();

    fn resolve_type(x: &syn::Ident) -> websocat_api::PropertyValueType {
        match &format!("{}", x)[..] {
            "i64" => websocat_api::PropertyValueType::Numbery,
            "NodeId" => websocat_api::PropertyValueType::ChildNode,
            "String" => websocat_api::PropertyValueType::Stringy,
            y => panic!("Unknown type {}", y),
        } 
    }

    match x.data {
        syn::Data::Struct(d) => {
            for f in d.fields {
                if let Some(fnam) = f.ident {
                    match f.ty {
                        syn::Type::Path(ty) => {
                            if let Some(x) = ty.path.segments.last() {
                                match &format!("{}", x.ident)[..] {
                                    "Option" => {
                                        match &x.arguments {
                                            syn::PathArguments::AngleBracketed(aa) => {
                                                match aa.args.last().unwrap() {
                                                    syn::GenericArgument::Type(syn::Type::Path(j)) => {
                                                        properties.push((fnam, resolve_type(&j.path.segments.last().unwrap().ident), true));
                                                    }
                                                    _ => panic!(),
                                                }
                                            }
                                            _ => panic!(),
                                        }
                                    }
                                    _ => properties.push((fnam, resolve_type(&x.ident), false)),
                                }
                            } else {
                                panic!("Cannot get last path segment");
                            }
                        }
                        _ => panic!("Only syn::Type::Path supported"),
                    }
                } else {
                    panic!("Fields must have names");
                }
            }
        }
        _ => panic!("Struct only"),
    }

    //eprintln!("{:?}", properties);
    let mut property_accessors = proc_macro2::TokenStream::new();

    for (nam, typ, opt) in &properties {
        let qn = format!("{}", nam);
        let typ = quote::format_ident!("{}", format!("{:?}", typ));
        if ! opt {
            property_accessors.extend(quote::quote! {
                #qn => Some(websocat_api::PropertyValue::#typ(self.#nam)),
            });
        } else {
            property_accessors.extend(quote::quote! {
                #qn => self.#nam.clone().map(websocat_api::PropertyValue::#typ),
            });
        }
    }

    let ts = quote::quote! {
        impl websocat_api::ParsedNodeProperyAccess for #name {
            fn class(&self) -> websocat_api::DNodeClass {
                Box::new(#classname)
            }
        
            fn get_property(&self, name:&str) -> Option<websocat_api::PropertyValue> {
                match name {
                    #property_accessors
                    _ => None,
                }
            }
        
            fn get_array(&self) -> Vec<websocat_api::PropertyValue> {
                vec![]
            }
        }        
    };
    ts.into()
}