use std::collections::BTreeSet;
use std::io::Write;

use log::debug;

type Error = Box<dyn std::error::Error + 'static>;

const CLASS_POPULATOR: &str = "auto_populate_in_allclasslist";
const MACRO_POPULATOR: &str = "auto_populate_macro_in_allclasslist";

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
        let mut cmd = std::process::Command::new("cargo");
        let cmd = cmd.args([
            "rustc",
            "--profile=check",
            "-p",
            cr.as_ref(),
            "--",
            "-Zunpretty=expanded",
        ]);
        let output = cmd.stdout(std::process::Stdio::piped()).output()?;
        if !output.status.success() {
            log::error!("Failed to obtain expanded source code of crate {}", cr);
            std::io::stderr().write_all(&output.stderr[..])?;
        }
        let content = String::from_utf8(output.stdout)?;
        let s: proc_macro2::TokenStream = content.parse()?;
        let t: syn::File = syn::parse2(s)?;
        //println!("{:#?}", t);
        walk(&cr, "", &t.items[..], &mut entries)?;
    }
    log::info!("Finished scanning");

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
        let k = e.krate.replace("-", "_");
        let path = format!("{}::{}", k, e.path);
        let method = match e.t {
            Type::Class => "register",
            Type::Macro => "register_macro",
        };
        src.write_all(format!("    reg.{method}::<{path}>();\n", method=method, path=path).as_bytes())?;
    }

    src.write_all(br##"    reg
}
"##)?;
    log::info!("Generated the src/lib.rs");

    Ok(())
}
