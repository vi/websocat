use std::collections::BTreeSet;
use std::io::Write;

use log::debug;

type Error = Box<dyn std::error::Error + 'static>;

const CLASS_POPULATOR: &str = "auto_populate_in_allclasslist";
const MACRO_POPULATOR: &str = "auto_populate_macro_in_allclasslist";

const CONFIG: &str = "classes.toml";

const BANNED_CRATES: [&str; 4] = [
    "websocat-api",
    "websocat-derive",
    "websocat-allnodes",
    "allnodes-codegen",
];

enum Type {
    Class,
    Macro,
}
struct Entry {
    t: Type,
    krate: String,
    path: String,
}

impl Entry {
    fn mangle_path(&self) -> String {
        self.path.replace("::", "-")
    }

    fn mangle_crate(&self) -> String {
        self.krate.replace("-", "_")
    }
}

fn walk(
    krate: &str,
    modpath: &str,
    items: &[syn::Item],
    output: &mut Vec<Entry>,
) -> Result<(), Error> {
    debug!("Entering module {}", modpath);
    for item in items {
        match item {
            syn::Item::Struct(s) => {
                let mut r#class = false;
                let mut r#macro = false;
                //let a = Lorem::from_attributes(&s.attrs[..]);
                for a in &s.attrs {
                    let l = &a.path.segments.last().unwrap().ident;
                    if l == CLASS_POPULATOR {
                        r#class = true;
                    }
                    if l == MACRO_POPULATOR {
                        r#macro = true;
                    }
                }

                debug!("A struct: {}", s.ident);
                if r#macro && r#class {
                    return Err(format!(
                        "Both macro-style and class-style auto-populate trigger is used for {}{}",
                        modpath, s.ident
                    ))?;
                }
                if r#class {
                    log::info!("  found a class {}{}", modpath, s.ident);
                    output.push(Entry {
                        t: Type::Class,
                        krate: krate.to_owned(),
                        path: format!("{}{}", modpath, s.ident),
                    });
                }
                if r#macro {
                    log::info!("  found a macro {}{}", modpath, s.ident);
                    output.push(Entry {
                        t: Type::Macro,
                        krate: krate.to_owned(),
                        path: format!("{}{}", modpath, s.ident),
                    });
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, ref subitems)) = m.content {
                    walk(krate, &format!("{}::", m.ident), &subitems[..], output)?;
                }
            }
            _ => (),
        }
    }
    debug!("Leaving module {}", modpath);
    Ok(())
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let mainman = cargo_toml::Manifest::from_path("Cargo.toml");
    let check = mainman
        .map(|x| match (x.package, x.workspace) {
            (None, Some(w)) => {
                debug!("the root package contains a worksapce");
                match w.members.iter().next().map(std::convert::AsRef::as_ref) {
                    Some("crates/*") => true,
                    _ => false,
                }
            }
            _ => false,
        })
        .unwrap_or(false);

    if !check {
        Err("Run this program from the root of Websocat project")?;
    }
    log::info!("Checked current directory");

    let mut crates: BTreeSet<String> = BTreeSet::new();

    for entry in walkdir::WalkDir::new("crates") {
        let e = entry?;
        if !e.file_type().is_file() {
            continue;
        }
        debug!("Considering {}", e.path().display());

        let cr = e.path().components().nth(1).unwrap();
        match cr {
            std::path::Component::Normal(x) => {
                let cn = x.to_str().unwrap();
                if BANNED_CRATES.contains(&cn) {
                    debug!("A banned crate");
                    continue;
                }
                if crates.contains(cn) {
                    debug!("Already registered this crate");
                    continue;
                }

                let content = std::fs::read_to_string(e.path())?;

                if content.contains(MACRO_POPULATOR) || content.contains(CLASS_POPULATOR) {
                    debug!("Registered candidate crate {}", cn);
                    crates.insert(cn.to_owned());
                } else {
                    debug!("This file does not contain anything interesting");
                }
            }
            _ => unreachable!(),
        }
    }

    let mut entries: Vec<Entry> = Vec::with_capacity(64);

    for cr in crates {
        log::info!("Scanning crate {}", cr);
        let path = format!("crates/{}/src/lib.rs", cr);
        let t: syn::File = syn_file_expand::read_full_crate_source_code(path, |_|Ok(true))?;
        //println!("{:#?}", t);
        walk(&cr, "", &t.items[..], &mut entries)?;
    }
    log::info!("Finished scanning");

    use toml::value::{Table, Value};

    let mut config = Table::with_capacity(32);
    if std::path::Path::new(CONFIG).exists() {
        log::info!("Reading config file");
        config = toml::de::from_str(&std::fs::read_to_string(CONFIG)?)?;
    } else {
        debug!("Config file does not exist");
    }

    let mut modified_config = false;
    for e in &entries {
        let t = config
            .entry(e.krate.to_owned())
            .or_insert_with(|| Value::Table(Table::with_capacity(16)));
        match t {
            Value::Table(t) => {
                t.entry(e.mangle_path()).or_insert_with(|| {
                    modified_config = true;
                    Value::String("default".to_owned())
                });
            }
            _ => unreachable!(),
        };
    }
    config.entry("default").or_insert_with(|| {
        modified_config = true;
        Value::String("builtin-default".to_owned())
    });

    if modified_config {
        std::fs::write(CONFIG, toml::ser::to_string_pretty(&config)?.as_bytes())?;
        log::info!("Written updated config file");
    }

    entries.retain(|e| {
        let status = &config[&e.krate].as_table().unwrap()[&e.mangle_path()];
        match status {
            Value::String(x) if x == "default" => match &config["default"] {
                Value::Boolean(x) => *x,
                Value::String(x) if x == "builtin-default" => true,
                _ => {
                    log::error!("Invalid setting `default` in the config file");
                    false
                }
            },
            Value::Boolean(x) => *x,
            _ => {
                log::error!(
                    "Invalid setting for {}::{} in the config file",
                    e.krate,
                    e.path
                );
                false
            }
        }
    });

    std::fs::create_dir_all("crates/websocat-allnodes/src")?;

    let crates = BTreeSet::from_iter(entries.iter().map(|x| x.krate.to_owned()));

    let mut ctml = std::fs::File::create("crates/websocat-allnodes/Cargo.toml")?;
    ctml.write_all(
        br##"# Note: this is an auto-generated file, but is intended to be in Git anyway

[package]
name = "websocat-allnodes"
version = "0.1.0"
edition = "2018"

[dependencies]
websocat-api = {path = "../websocat-api", features=["sync_impl"]}

"##,
    )?;
    for cr in &crates {
        ctml.write_all(format!("{cr} = {{path = \"../{cr}\"}}\n", cr = cr).as_bytes())?;
    }
    drop(ctml);

    log::info!("Generated the Cargo.toml");

    let mut src = std::fs::File::create("crates/websocat-allnodes/src/lib.rs")?;
    src.write_all(br##"//! This is an auto-generated file based on auto_populate_in_allclasslist annotations, but it is intended to be in Git anyway

/// Get `ClassRegistrar` with all WebSocat's nodes registered
pub fn all_node_classes() -> websocat_api::ClassRegistrar {
    let mut reg = websocat_api::ClassRegistrar::default();

"##)?;

    for e in &entries {
        let k = e.mangle_crate();
        let path = format!("{}::{}", k, e.path);
        let method = match e.t {
            Type::Class => "register",
            Type::Macro => "register_macro",
        };
        src.write_all(
            format!(
                "    reg.{method}::<{path}>();\n",
                method = method,
                path = path
            )
            .as_bytes(),
        )?;
    }

    src.write_all(
        br##"    reg
}
"##,
    )?;
    log::info!("Generated the src/lib.rs");

    Ok(())
}
