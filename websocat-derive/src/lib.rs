extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

use quote::quote as q;

#[derive(Debug, darling::FromField)]
#[darling(forward_attrs(doc))]
struct Field1 {
    ident:	Option<syn::Ident>,
    ty:	syn::Type,

    attrs: Vec<syn::Attribute>,
}

#[derive(Debug, darling::FromDeriveInput)]
#[darling(attributes(websocat_node, debug_derive), forward_attrs(doc, official_name))]
struct Class1 {
    ident: syn::Ident,
    data: darling::ast::Data<(),Field1>,
    official_name: String,

    #[darling(multiple, rename="prefix")]
    prefixes: Vec<String>,

    #[darling(default)]
    debug_derive: bool,
}


struct PropertyInfo {
    ident: syn::Ident,
    typ : websocat_api::PropertyValueType,
    optional: bool,
    help: String,
}
struct ClassInfo {
    name: syn::Ident,
    properties: Vec<PropertyInfo>,

    official_name: String,
    prefixes: Vec<String>,  

    debug_derive: bool,
}

fn proptype(x: &websocat_api::PropertyValueType) -> proc_macro2::TokenStream {
    match x {
        websocat_api::PropertyValueType::Stringy => q!{::std::string::String},
        websocat_api::PropertyValueType::Enummy(_) => panic!("enums not implemented"),
        websocat_api::PropertyValueType::Numbery => q!{i64},
        websocat_api::PropertyValueType::Floaty => q!{f64},
        websocat_api::PropertyValueType::Booly => q!{bool},
        websocat_api::PropertyValueType::SockAddr => q!{::std::net::SockAddr},
        websocat_api::PropertyValueType::IpAddr => q!{::std::net::IpAddr},
        websocat_api::PropertyValueType::PortNumber => q!{u16},
        websocat_api::PropertyValueType::Path => q!{::std::path::PathBuf},
        websocat_api::PropertyValueType::Uri => q!{::websocat_api::http::Uri},
        websocat_api::PropertyValueType::Duration => q!{::std::time::Duration},
        websocat_api::PropertyValueType::ChildNode => q!{::websocat_api::NodeId},
    }
}

fn resolve_type(x: &syn::Ident) -> websocat_api::PropertyValueType {
    match &format!("{}", x)[..] {
        "i64" => websocat_api::PropertyValueType::Numbery,
        "NodeId" => websocat_api::PropertyValueType::ChildNode,
        "String" => websocat_api::PropertyValueType::Stringy,
        y => panic!("Unknown type {}", y),
    } 
}

trait PVTHelper {
    fn ident(&self) -> proc_macro2::TokenStream;
}
impl PVTHelper for websocat_api::PropertyValueType {
    fn ident(&self) -> proc_macro2::TokenStream {
        macro_rules! w {
            ($($x:ident,)*) => {
                match self {
                    ::websocat_api::PropertyValueType::Enummy(_) => panic!("enums not implemented"),
                    $(
                        ::websocat_api::PropertyValueType::$x => q!{$x},
                    )*
                }
            }
        }
        w!(
            Stringy,
            Numbery,
            Floaty,
            Booly,
            SockAddr,
            IpAddr,
            PortNumber,
            Path,
            Uri,
            Duration,
            ChildNode,
        )
    }
}

impl ClassInfo {
    pub fn parse(x: &syn::DeriveInput) -> ClassInfo {
        use darling::FromDeriveInput;

        let cc = Class1::from_derive_input(x).unwrap();

        let mut properties: Vec<PropertyInfo> = vec![];
        
        if cc.debug_derive {
            let mut f = std::fs::File::create("/tmp/derive.txt").unwrap();
            use std::io::Write;
            writeln!(f, "{:#?}", cc).unwrap();
        }

        match cc.data {
            darling::ast::Data::Enum(_) => panic!("Enums are not supported"),
            darling::ast::Data::Struct(x) => {
                for field in x {
                    //eprintln!("{:?}", field);
                    let ident = field.ident.expect("Struct fields must have names");
                    let (typ, optional) = match field.ty {
                        syn::Type::Path(t) => {
                            let lastpathsegment = t.path.segments.last().expect("Failed to extract leaf type from path in a field");
                            match &lastpathsegment.ident.to_string()[..] {
                                "Result" => panic!("`Result`s are not supported"),
                                "Option" => {
                                    match &lastpathsegment.arguments {
                                        syn::PathArguments::AngleBracketed(aa) => {
                                            match aa.args.last().expect("Failed to extract leaf type from within an Option") {
                                                syn::GenericArgument::Type(syn::Type::Path(p)) => {
                                                    (resolve_type(&p.path.segments.last().unwrap().ident), true)
                                                }
                                                _ => panic!("Option should have a normal type inside it, not something else"),
                                            }
                                        }
                                        _ => panic!(),
                                    }
                                }
                                _ => (resolve_type(&lastpathsegment.ident), false),
                            }
                        },
                        _ => panic!("Unknown type for field named {}", ident),
                    };
                    let mut help = String::with_capacity(64);
                    for attr in &field.attrs {
                        if attr.path.segments.last().unwrap().ident == "doc" {
                            match attr.tokens.clone().into_iter().last() {
                                Some(proc_macro2::TokenTree::Literal(l)) => {
                                    match syn::Lit::new(l) {
                                        syn::Lit::Str(ll) => {
                                            if ! help.is_empty() {
                                                help += &"\n";
                                            }
                                            help += &ll.value();
                                        }
                                        _ => panic!("doc attribute is not a string literal"),
                                    }
                                }
                                _ => panic!("doc attribute is not a string literal"),
                            }
                        }
                    }
                    if help.is_empty() {
                        panic!("Undocumented field: {}", &ident);
                    }
                    properties.push(PropertyInfo {
                        help,
                        typ,
                        optional,
                        ident,
                    });
                }
            }
        }
        
        let ci = ClassInfo {
            name: x.ident.clone(),
            properties,
            prefixes: cc.prefixes,
            official_name: cc.official_name,
            debug_derive: cc.debug_derive,
        };
        ci
    } 


    #[allow(non_snake_case)]
    fn generate_ParsedNodeProperyAccess(&self) -> proc_macro2::TokenStream {
        let ci = self;
        let mut property_accessors = proc_macro2::TokenStream::new();

        let classname = quote::format_ident!("{}Class", ci.name);

        for p in &ci.properties {
            let nam = &p.ident;
            let qn = format!("{}", p.ident);
            let typ = p.typ.ident();
            if ! p.optional {
                property_accessors.extend(q! {
                    #qn => Some(::websocat_api::PropertyValue::#typ(self.#nam)),
                });
            } else {
                property_accessors.extend(q! {
                    #qn => self.#nam.clone().map(::websocat_api::PropertyValue::#typ),
                });
            }
        }
    
        let name = &ci.name;
        let ts = q! {
            impl ::websocat_api::ParsedNodeProperyAccess for #name {
                fn class(&self) -> ::websocat_api::DNodeClass {
                    Box::new(#classname)
                }
            
                fn get_property(&self, name:&str) -> ::std::option::Option<::websocat_api::PropertyValue> {
                    match name {
                        #property_accessors
                        _ => None,
                    }
                }
            
                fn get_array(&self) -> ::std::vec::Vec<::websocat_api::PropertyValue> {
                    vec![]
                }
                
                fn clone(&self) -> ::websocat_api::DParsedNode {
                    ::std::boxed::Box::pin(::std::clone::Clone::clone(self))
                }
            }        
        };
        ts
    }

    fn generate_builder(&self) -> proc_macro2::TokenStream {
        let ci = self;
        
        let buildername = quote::format_ident!("{}Builder", ci.name);
        let mut fields = proc_macro2::TokenStream::new();

        for p in &ci.properties {
            let nam = &p.ident;
            let typ = proptype(&p.typ);
            fields.extend(q! {
                #nam : ::std::option::Option<#typ>,
            });
        }
    
        let ts = q! {
            #[derive(Default)]
            struct #buildername {
                #fields
            }
        };
        ts
    }


    #[allow(non_snake_case)]
    fn generate_NodeInProgressOfParsing(&self) -> proc_macro2::TokenStream {
        let buildername = quote::format_ident!("{}Builder", self.name);
        let name = &self.name;

        let mut none_checks =  proc_macro2::TokenStream::new();
        let mut fields=  proc_macro2::TokenStream::new();
        let mut matchers=  proc_macro2::TokenStream::new();
        
        for p in &self.properties {
            let pn = &p.ident;
            let pn_s = pn.to_string();
            if ! p.optional {
                let name_s = name.to_string();

                none_checks.extend(q! {
                    if self.#pn.is_none() {
                        ::websocat_api::anyhow::bail!(
                            "Property `{}` must be set in node of type `{}`",
                            #pn_s,
                            #name_s,
                        );
                    }
                });
                fields.extend(q! {
                    #pn : self.#pn.unwrap(),
                });
            } else {
                fields.extend(q! {
                    #pn : self.#pn,
                });
            }

            let pty = p.typ.ident();

            matchers.extend(q! {
                (#pn_s, ::websocat_api::PropertyValue::#pty(n)) => self.#pn = ::std::option::Option::Some(n),
            })

        }

        let ts = q! {          
            impl ::websocat_api::NodeInProgressOfParsing for #buildername {
                fn set_property(&mut self, name: &str, val: ::websocat_api::PropertyValue) -> ::websocat_api::Result<()> {
                    match (name, val) {
                        #matchers
                        _ => ::websocat_api::anyhow::bail!("Unknown property {} or wrong type", name),
                    }
                    Ok(())
                }

                fn push_array_element(&mut self, val: ::websocat_api::PropertyValue) -> ::websocat_api::Result<()> {
                    ::websocat_api::anyhow::bail!("No array elements expected here");
                }

                fn finish(self: Box<Self>) -> ::websocat_api::Result<websocat_api::DParsedNode> {
                    #none_checks
                    ::std::result::Result::Ok(::std::boxed::Box::pin(
                        #name {
                            #fields
                        }
                    ))
                }
            }
        };
        ts
    }

    #[allow(non_snake_case)]
    fn generate_NodeClass(&self) -> proc_macro2::TokenStream {
        let offiname = &self.official_name;

        let mut property_infos =  proc_macro2::TokenStream::new();
        
        for p in &self.properties {
            let pn = &p.ident;
            let pn_s = pn.to_string();
            let help = &p.help;
            let pt = p.typ.ident();

            property_infos.extend(q! {
                ::websocat_api::PropertyInfo {
                    name: #pn_s.to_owned(),
                    r#type: websocat_api::PropertyValueType::#pt,
                    help: #help.to_owned(),
                },
            })
        }

        let mut prefixes = proc_macro2::TokenStream::new();

        for pr in &self.prefixes {
            prefixes.extend(q!{
                #pr.to_owned(),
            });
        }

        let buildername = quote::format_ident!("{}Builder", self.name);
        let classname = quote::format_ident!("{}Class", self.name);
        let name = &self.name;

        let ts = q! {    
            #[derive(Default)]      
            struct #classname;

            impl ::websocat_api::NodeClass for #classname {
                fn official_name(&self) -> ::std::string::String { #offiname.to_owned() }
            
                fn prefixes(&self) -> ::std::vec::Vec<::std::string::String> { vec![
                    #prefixes
                ] }
            
                fn properties(&self) -> ::std::vec::Vec<::websocat_api::PropertyInfo> {
                    vec![
                        #property_infos
                    ]
                }
            
                fn array_type(&self) -> ::std::option::Option<::websocat_api::PropertyValueType> {
                    None
                }
            
                fn new_node(&self) -> ::websocat_api::DNodeInProgressOfParsing {
                    ::std::boxed::Box::new(#buildername::default())
                }
            
                fn run_lints(&self, nodeid: &::websocat_api::NodeId, placement: ::websocat_api::NodePlacement, context: &::websocat_api::WebsocatContext) -> ::websocat_api::Result<::std::vec::Vec<::std::string::String>> {
                    Ok(vec![])
                }
            }

            impl ::websocat_api::GetClassOfNode for #name {
                type Class = #classname;
            }
        };
        ts
    }
}

#[proc_macro_derive(WebsocatNode, attributes(websocat_node))]
pub fn derive_websocat_node(input: TokenStream) -> TokenStream {
    let x = parse_macro_input!(input as DeriveInput);
    let ci = ClassInfo::parse(&x);
    
    let mut code = proc_macro2::TokenStream::new();

    code.extend(ci.generate_ParsedNodeProperyAccess());
    code.extend(ci.generate_builder());
    code.extend(ci.generate_NodeInProgressOfParsing());
    code.extend(ci.generate_NodeClass());
    
    if ci.debug_derive {
        use std::io::Write;
        let mut f = std::fs::File::create("/tmp/derive.rs").unwrap();
        writeln!(f, "{}", code).unwrap();
    }

    code.into()
}