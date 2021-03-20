extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use quote::quote as q;

#[derive(Debug, darling::FromField)]
#[darling(attributes(websocat_node, enum), forward_attrs(doc,cli))]
struct Field1 {
    ident:	Option<syn::Ident>,
    ty:	syn::Type,

    attrs: Vec<syn::Attribute>,

    #[darling(default, rename="enum")]
    r#enum: bool,
}

#[derive(Debug, darling::FromDeriveInput)]
#[darling(attributes(websocat_node, debug_derive), forward_attrs(doc, official_name, validate))]
struct Class1 {
    ident: syn::Ident,
    data: darling::ast::Data<(),Field1>,
    official_name: String,

    #[darling(multiple, rename="prefix")]
    prefixes: Vec<String>,

    #[darling(default)]
    debug_derive: bool,

    #[darling(default)]
    validate: bool,
}

#[derive(Debug)]
struct PropertyInfo {
    ident: syn::Ident,
    typ : websocat_api::PropertyValueTypeTag,
    enumname: Option<syn::TypePath>,
    optional: bool,
    help: String,

    pub inject_cli_long_option: Option<String>,
}
#[derive(Debug)]
struct ClassInfo {
    name: syn::Ident,
    properties: Vec<PropertyInfo>,
    array_type: Option<PropertyInfo>,

    official_name: String,
    prefixes: Vec<String>,  
    validate: bool,

    debug_derive: bool,
}

fn proptype(x: &websocat_api::PropertyValueTypeTag, enbt: &Option<syn::TypePath>) -> proc_macro2::TokenStream {
    match x {
        websocat_api::PropertyValueTypeTag::Stringy => q!{::std::string::String},
        websocat_api::PropertyValueTypeTag::Enummy => {
            let enbt = enbt.as_ref().unwrap();
            q! {  # enbt }
        }
        websocat_api::PropertyValueTypeTag::Numbery => q!{i64},
        websocat_api::PropertyValueTypeTag::Floaty => q!{f64},
        websocat_api::PropertyValueTypeTag::Booly => q!{bool},
        websocat_api::PropertyValueTypeTag::SockAddr => q!{::std::net::SocketAddr},
        websocat_api::PropertyValueTypeTag::IpAddr => q!{::std::net::IpAddr},
        websocat_api::PropertyValueTypeTag::PortNumber => q!{u16},
        websocat_api::PropertyValueTypeTag::Path => q!{::std::path::PathBuf},
        websocat_api::PropertyValueTypeTag::Uri => q!{::websocat_api::http::Uri},
        websocat_api::PropertyValueTypeTag::Duration => q!{::std::time::Duration},
        websocat_api::PropertyValueTypeTag::ChildNode => q!{::websocat_api::NodeId},
    }
}

fn resolve_type(x: &syn::Ident) -> websocat_api::PropertyValueTypeTag {
    match &format!("{}", x)[..] {
        "i64" => websocat_api::PropertyValueTypeTag::Numbery,
        "f64" => websocat_api::PropertyValueTypeTag::Floaty,
        "NodeId" => websocat_api::PropertyValueTypeTag::ChildNode,
        "String" => websocat_api::PropertyValueTypeTag::Stringy,
        "SocketAddr" => websocat_api::PropertyValueTypeTag::SockAddr,
        "u16" => websocat_api::PropertyValueTypeTag::PortNumber,
        "bool" => websocat_api::PropertyValueTypeTag::Booly,
        y => panic!("Unknown type {}", y),
    } 
}

trait PVTHelper {
    fn ident(&self) -> proc_macro2::TokenStream;
}
impl PVTHelper for websocat_api::PropertyValueTypeTag {
    fn ident(&self) -> proc_macro2::TokenStream {
        macro_rules! w {
            ($($x:ident,)*) => {
                match self {
                    ::websocat_api::PropertyValueTypeTag::Enummy => panic!("PVTHelper::ident should not be called for enummys"),
                    $(
                        ::websocat_api::PropertyValueTypeTag::$x => q!{$x},
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
        let mut array_type: Option<PropertyInfo> = None;
        
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
                    let (typ, optional, enumname,vector) = match field.ty {
                        syn::Type::Path(t) => {
                            let lastpathsegment = t.path.segments.last().expect("Failed to extract leaf type from path in a field");
                            match &lastpathsegment.ident.to_string()[..] {
                                "Result" => panic!("`Result`s are not supported"),
                                "HashSet" => panic!("`HashSet`s are not supported"),
                                "Option" => {
                                    match &lastpathsegment.arguments {
                                        syn::PathArguments::AngleBracketed(aa) => {
                                            match aa.args.last().expect("Failed to extract leaf type from within an Option") {
                                                syn::GenericArgument::Type(syn::Type::Path(p)) => {
                                                    if field.r#enum {
                                                        (websocat_api::PropertyValueTypeTag::Enummy, true, Some(p.clone()), false)
                                                    } else {
                                                        (resolve_type(&p.path.segments.last().unwrap().ident), true, None, false)
                                                    }
                                                }
                                                _ => panic!("Option should have a normal type inside it, not something else"),
                                            }
                                        }
                                        _ => panic!(),
                                    }
                                }
                                "Vec" => {
                                    match &lastpathsegment.arguments {
                                        syn::PathArguments::AngleBracketed(aa) => {
                                            match aa.args.last().expect("Failed to extract leaf type from within an Vec") {
                                                syn::GenericArgument::Type(syn::Type::Path(p)) => {
                                                    if field.r#enum {
                                                        (
                                                            websocat_api::PropertyValueTypeTag::Enummy,
                                                            false,
                                                            Some(p.clone()),
                                                            true,
                                                        )
                                                    } else {
                                                        (
                                                            resolve_type(&p.path.segments.last().unwrap().ident),
                                                            false,
                                                            None,
                                                            true,
                                                        )
                                                    }
                                                }
                                                _ => panic!("Vec should have a normal type inside it, not something else"),
                                            }
                                        }
                                        _ => panic!(),
                                    }
                                }
                                _ => if field.r#enum {
                                    (websocat_api::PropertyValueTypeTag::Enummy, false, Some(t.clone()), false)
                                } else {
                                    (resolve_type(&lastpathsegment.ident), false, None, false)
                                },
                            }
                        },
                        _ => panic!("Unknown type for field named {}", ident),
                    };
                    let mut help = String::with_capacity(64);
                    let mut inject_cli_long_option = None;
                    for attr in &field.attrs {
                        let name = &attr.path.segments.last().unwrap().ident;
                        if name == "doc" || name == "cli" {
                            match attr.tokens.clone().into_iter().last() {
                                Some(proc_macro2::TokenTree::Literal(l)) => {
                                    match syn::Lit::new(l) {
                                        syn::Lit::Str(ll) => {
                                            if name == "doc" {
                                                if ! help.is_empty() {
                                                    help += &"\n";
                                                }
                                                help += &ll.value();
                                            } else if name == "cli" {
                                                inject_cli_long_option = Some(ll.value());
                                            }
                                        }
                                        _ => panic!("doc or cli attribute is not a string literal"),
                                    }
                                }
                                _ => panic!("doc or cli attribute is not a string literal"),
                            }
                        }
                    }
                    if help.is_empty() {
                        panic!("Undocumented field: {}", &ident);
                    }
                    if vector {
                        if array_type.is_some() {
                            panic!("There can only be one array per node");
                        }
                        assert!(!optional);
                        array_type = Some(PropertyInfo {
                            ident,
                            typ,
                            enumname,
                            optional: false,
                            help,
                            inject_cli_long_option: None,

                        });
                    } else { 
                        if typ == websocat_api::PropertyValueTypeTag::PortNumber {
                            if format!("{}", ident).to_lowercase().contains("port") {
                                // OK
                            } else {
                                panic!("u16 types should only be used for port numbers. Mention the substring `port` in the field name.")
                            }
                        }
                        properties.push(PropertyInfo {
                            help,
                            typ,
                            optional,
                            ident,
                            enumname,
                            inject_cli_long_option,
                        });
                    }
                }
            }
        }
        
        let ci = ClassInfo {
            name: x.ident.clone(),
            properties,
            array_type,
            prefixes: cc.prefixes,
            official_name: cc.official_name,
            debug_derive: cc.debug_derive,
            validate: cc.validate,
        };

        if cc.debug_derive {
            let mut f = std::fs::File::create("/tmp/derive2.txt").unwrap();
            use std::io::Write;
            writeln!(f, "{:#?}", ci).unwrap();
        }

        ci
    } 


    #[allow(non_snake_case)]
    fn generate_NodeProperyAccess(&self) -> proc_macro2::TokenStream {
        let ci = self;
        let mut property_accessors = proc_macro2::TokenStream::new();
        let mut array_accessor = proc_macro2::TokenStream::new();

        let classname = quote::format_ident!("{}Class", ci.name);

        for p in &ci.properties {
            let nam = &p.ident;
            let qn = format!("{}", p.ident);
            if p.typ != websocat_api::PropertyValueTypeTag::Enummy {
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
            } else {
                //let enn = p.enumname.as_ref().unwrap();
                if ! p.optional {
                    property_accessors.extend(q! {
                        #qn => Some(::websocat_api::PropertyValue::Enummy(::websocat_api::Enum::variant_to_index(&self.#nam))),
                    });
                } else {
                    property_accessors.extend(q! {
                        #qn => self.#nam.as_ref().map(::websocat_api::Enum::variant_to_index).map(::websocat_api::PropertyValue::Enummy),
                    });
                }
            }
        }

        if let Some(p) = &ci.array_type {
            let nam = &p.ident;
            if p.typ == websocat_api::PropertyValueTypeTag::Enummy {
                array_accessor.extend(q!{
                    self.#nam.iter().map(|x|::websocat_api::PropertyValue::Enummy(::websocat_api::Enum::variant_to_index(x))).collect()
                });
            } else {
                let typ = p.typ.ident();
                array_accessor.extend(q!{
                    self.#nam.iter().map(|x| ::websocat_api::PropertyValue::#typ(x.clone())).collect()
                });
            }
        } else {
            array_accessor.extend(q!{
                vec![]
            });
        }
    
        let name = &ci.name;
        let ts = q! {
            impl ::websocat_api::NodeProperyAccess for #name {
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
                    #array_accessor
                }
                
                fn deep_clone(&self) -> ::websocat_api::DNode {
                    ::std::sync::Arc::pin(::std::clone::Clone::clone(self))
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
            let typ = proptype(&p.typ, &p.enumname);
            fields.extend(q! {
                #nam : ::std::option::Option<#typ>,
            });
        }

        if let Some(a) = &ci.array_type {
            let nam = &a.ident;
            let typ = proptype(&a.typ, &a.enumname);
            fields.extend(q! {
                #nam : ::std::vec::Vec<#typ>,
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
        let mut push_array_element = proc_macro2::TokenStream::new();
        
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

            if p.typ != websocat_api::PropertyValueTypeTag::Enummy {
                let pty = p.typ.ident();

                matchers.extend(q! {
                    (#pn_s, ::websocat_api::PropertyValue::#pty(n)) => self.#pn = ::std::option::Option::Some(n),
                })
            } else {
                let enn = p.enumname.as_ref().unwrap();
                matchers.extend(q! {
                    (#pn_s, ::websocat_api::PropertyValue::Enummy(sym)) => self.#pn = ::std::option::Option::Some({
                        <#enn as ::websocat_api::Enum>::index_to_variant(sym)
                    }),
                })
            }
        }

        if let Some(p) = &self.array_type {
            let pn = &p.ident;
            fields.extend(q!{
                #pn: self.#pn,
            });
            if p.typ != websocat_api::PropertyValueTypeTag::Enummy {
                let pty = p.typ.ident();

                push_array_element.extend(q! {
                    match val {
                        ::websocat_api::PropertyValue::#pty(x) => self.#pn.push(x),
                        _ => ::websocat_api::anyhow::bail!("Attempt to push wrong valued element to node's array"),
                    }
                    Ok(())
                });
            } else {
                let enn = p.enumname.as_ref().unwrap();
                push_array_element.extend(q! {
                    match val {
                        ::websocat_api::PropertyValue::Enummy(sym) => self.#pn.push(<#enn as ::websocat_api::Enum>::index_to_variant(sym)),
                        _ => ::websocat_api::anyhow::bail!("Attempt to push wrong valued element to node's array"),
                    }
                    Ok(())
                });
            }
        } else {
            push_array_element.extend(q! {
                ::websocat_api::anyhow::bail!("No array elements are expected here");
            });
        }

        let mut validate = proc_macro2::TokenStream::new();

        if self.validate {
            validate.extend(q!{
                x.validate()?;
            });
        }

        let ts = q! {          
            impl ::websocat_api::NodeInProgressOfParsing for #buildername {
                #[allow(unreachable_code)]
                fn set_property(&mut self, name: &str, val: ::websocat_api::PropertyValue) -> ::websocat_api::Result<()> {
                    match (name, val) {
                        #matchers
                        _ => ::websocat_api::anyhow::bail!("Unknown property {} or wrong type", name),
                    }
                    Ok(())
                }

                fn push_array_element(&mut self, val: ::websocat_api::PropertyValue) -> ::websocat_api::Result<()> {
                    #push_array_element
                }

                fn finish(self: Box<Self>) -> ::websocat_api::Result<websocat_api::DNode> {
                    #none_checks
                    let mut x = #name {
                        #fields
                    };
                    #validate
                    ::std::result::Result::Ok(::std::sync::Arc::pin(
                        x
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

        let mut array_type =  proc_macro2::TokenStream::new();
        let mut array_help =  proc_macro2::TokenStream::new();
        
        for p in &self.properties {
            let pn = &p.ident;
            let pn_s = pn.to_string();
            let help = &p.help;
            let iclo = if let Some(ref x) = p.inject_cli_long_option {
                q!{::std::option::Option::Some(#x.to_owned())}
            } else {
                q!{::std::option::Option::None}
            };
            if p.typ != websocat_api::PropertyValueTypeTag::Enummy {
                let pt = p.typ.ident();

                property_infos.extend(q! {
                    ::websocat_api::PropertyInfo {
                        name: #pn_s.to_owned(),
                        r#type: websocat_api::PropertyValueType::#pt,
                        help: ::std::boxed::Box::new(||#help.to_owned()),
                        inject_cli_long_option: #iclo,
                    },
                })
            } else {
                let enn = p.enumname.as_ref().unwrap();
                property_infos.extend(q! {
                    ::websocat_api::PropertyInfo {
                        name: #pn_s.to_owned(),
                        r#type: websocat_api::PropertyValueType::Enummy(<#enn as ::websocat_api::Enum>::interner()),
                        help: ::std::boxed::Box::new(||#help.to_owned()),
                        inject_cli_long_option: #iclo,
                    },
                })
            }
        }

        if let Some(p) = &self.array_type {
            let help = &p.help;
            if p.typ != websocat_api::PropertyValueTypeTag::Enummy {
                let pt = p.typ.ident();
                array_type.extend(q! {
                    Some(websocat_api::PropertyValueType::#pt)
                })
            } else {
                let enn = p.enumname.as_ref().unwrap();
                array_type.extend(q! {
                    Some(websocat_api::PropertyValueType::Enummy(<#enn as ::websocat_api::Enum>::interner()))
                })
            }
            array_help.extend(q!{
                Some(#help.to_owned())
            })
        } else {
            array_type.extend(q!{ None });
            array_help.extend(q!{ None });
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
            #[derive(Default,Debug)]      
            pub struct #classname;

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
                    #array_type
                }
                fn array_help(&self) -> ::std::option::Option<::std::string::String> {
                    #array_help
                }
            
                fn new_node(&self) -> ::websocat_api::DNodeInProgressOfParsing {
                    ::std::boxed::Box::new(#buildername::default())
                }
            
                fn run_lints(&self, nodeid: ::websocat_api::NodeId, placement: ::websocat_api::NodePlacement, context: &::websocat_api::Session) -> ::websocat_api::Result<::std::vec::Vec<::std::string::String>> {
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

#[proc_macro_derive(WebsocatNode, attributes(websocat_node,cli))]
pub fn derive_websocat_node(input: TokenStream) -> TokenStream {
    let x = parse_macro_input!(input as DeriveInput);
    let ci = ClassInfo::parse(&x);
    
    let mut code = proc_macro2::TokenStream::new();

    code.extend(ci.generate_NodeProperyAccess());
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



#[derive(Debug, darling::FromVariant)]
#[darling(attributes(websocat_enum, rename))]
struct EnummyVariant {
    ident:	syn::Ident,

    #[darling(default)]
    rename: Option<String>,
}

impl EnummyVariant {
    fn get_name(&self, lowercase: bool) -> String {
        if let Some(ref x) = self.rename {
            x.to_owned()
        } else if lowercase {
            self.ident.to_string().to_lowercase()
        } else {
            format!("{}", self.ident)
        }
    }
}

#[derive(Debug, darling::FromDeriveInput)]
#[darling(attributes(websocat_enum, rename_all_lowercase, debug_derive))]
struct EnummyEnum {
    ident: syn::Ident,
    data: darling::ast::Data<EnummyVariant,()>,

    #[darling(default)]
    rename_all_lowercase: bool,

    #[darling(default)]
    debug_derive: bool,
}


#[proc_macro_derive(WebsocatEnum, attributes(websocat_enum))]
pub fn derive_websocat_enum(input: TokenStream) -> TokenStream {
    let x = parse_macro_input!(input as DeriveInput);
    use darling::FromDeriveInput;
    let cc = EnummyEnum::from_derive_input(&x).unwrap();
    
    if cc.debug_derive {
        let mut f = std::fs::File::create("/tmp/derive.txt").unwrap();
        use std::io::Write;
        writeln!(f, "{:#?}", cc).unwrap();
    }
    
    let name = cc.ident;
    let namestr = format!("{}", name);


    let mut interner_filler = proc_macro2::TokenStream::new();
    let mut variant_count : usize = 0;

    let mut variant_to_index_match = proc_macro2::TokenStream::new();
    let mut index_to_variant_match = proc_macro2::TokenStream::new();

    match cc.data {
        darling::ast::Data::Struct(_) => panic!("WebsocatEnum expects only enums, not structs"),
        darling::ast::Data::Enum(x) => {
            for (n, variant) in x.iter().enumerate() {
                let varname = variant.get_name(cc.rename_all_lowercase);
                variant_count += 1;

                interner_filler.extend(q! {
                    assert_eq!(s.get_or_intern(#varname), ::websocat_api::string_interner::DefaultSymbol::try_from_usize(#n).unwrap());
                });

                let identnam = &variant.ident;
                index_to_variant_match.extend(q!{
                    #n => Self::#identnam,
                });

                variant_to_index_match.extend(q!{
                    Self::#identnam => ::websocat_api::string_interner::DefaultSymbol::try_from_usize(#n).unwrap(),
                });
            }
        }
    }


    let mut code = proc_macro2::TokenStream::new();

    code.extend(q!{
        impl ::websocat_api::Enum for #name {
            fn interner() -> ::websocat_api::string_interner::StringInterner {
                use ::websocat_api::string_interner::Symbol;
                let mut s = ::websocat_api::string_interner::StringInterner::with_capacity(#variant_count);
                #interner_filler
                s
            }
        
            fn index_to_variant(sym: ::websocat_api::string_interner::DefaultSymbol) -> Self {
                use ::websocat_api::string_interner::Symbol;
                match sym.to_usize() {
                    #index_to_variant_match
                    x => panic!("Invalid numeric value {} for enummy {}", x, #namestr),
                }
            }

            fn variant_to_index(&self) -> ::websocat_api::string_interner::DefaultSymbol {
                use ::websocat_api::string_interner::Symbol;
                match self {
                    #variant_to_index_match
                }
            }
        }
    });
    
    if cc.debug_derive {
        use std::io::Write;
        let mut f = std::fs::File::create("/tmp/derive.rs").unwrap();
        writeln!(f, "{}", code).unwrap();
    }

    code.into()
}
