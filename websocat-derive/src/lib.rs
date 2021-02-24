extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};


struct PropertyInfo {
    ident: syn::Ident,
    typ : websocat_api::PropertyValueType,
    optional: bool,
}
struct ClassInfo {
    name: syn::Ident,
    properties: Vec<PropertyInfo>,
}

impl ClassInfo {
    pub fn parse(x: &syn::DeriveInput) -> ClassInfo {
        let mut ci = ClassInfo {
            properties : vec![],
            name: x.ident.clone(),
        };

        fn resolve_type(x: &syn::Ident) -> websocat_api::PropertyValueType {
            match &format!("{}", x)[..] {
                "i64" => websocat_api::PropertyValueType::Numbery,
                "NodeId" => websocat_api::PropertyValueType::ChildNode,
                "String" => websocat_api::PropertyValueType::Stringy,
                y => panic!("Unknown type {}", y),
            } 
        }
    
        match &x.data {
            syn::Data::Struct(d) => {
                for f in &d.fields {
                    if let Some(fnam) = &f.ident {
                        match &f.ty {
                            syn::Type::Path(ty) => {
                                if let Some(x) = ty.path.segments.last() {
                                    match &format!("{}", x.ident)[..] {
                                        "Option" => {
                                            match &x.arguments {
                                                syn::PathArguments::AngleBracketed(aa) => {
                                                    match aa.args.last().unwrap() {
                                                        syn::GenericArgument::Type(syn::Type::Path(j)) => {
                                                            ci.properties.push(PropertyInfo {
                                                                ident: fnam.clone(),
                                                                typ: resolve_type(&j.path.segments.last().unwrap().ident),
                                                                optional: true,
    
                                                            });
                                                        }
                                                        _ => panic!(),
                                                    }
                                                }
                                                _ => panic!(),
                                            }
                                        }
                                        _ => {
                                            ci.properties.push(PropertyInfo{
                                                ident: fnam.clone(),
                                                typ:  resolve_type(&x.ident),
                                                optional: false,
                                            })
                                        }
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

        ci
    } 


    #[allow(non_snake_case)]
    fn generate_ParsedNodeProperyAccess(&self) -> proc_macro2::TokenStream {
        let ci = self;
        let mut property_accessors = proc_macro2::TokenStream::new();

        let classname = quote::format_ident!("{}Class", ci.name);

        for PropertyInfo{ident:nam, typ, optional:opt} in &ci.properties {
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
    
        let name = &ci.name;
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
        ts
    }
}

#[proc_macro_derive(WebsocatNode, attributes(name))]
pub fn derive_websocat_node(input: TokenStream) -> TokenStream {
    let x = parse_macro_input!(input as DeriveInput);

    let mut f = std::fs::File::create("/tmp/derive.txt").unwrap();
    use std::io::Write;
    writeln!(f, "{:#?}", x).unwrap();

    let ci = ClassInfo::parse(&x);
    
    let mut code = proc_macro2::TokenStream::new();

    code.extend(ci.generate_ParsedNodeProperyAccess());

    code.into()
}