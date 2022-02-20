#![allow(unused)]

use std::{borrow::Cow, collections::{BTreeMap, HashMap}};
use websocat_api::{ClassRegistrar, DMacro, DNodeClass, PropertyValueType};
pub enum HelpMode {
    Full,
    Short,
    Man,
    Markdown,
    JustListThings,
    SpecificThing(String),
}

fn format_pvt(t: &PropertyValueType) -> Cow<'static, str> {
    match t {
        PropertyValueType::Stringy => Cow::Borrowed("string"),
        PropertyValueType::BytesBuffer => Cow::Borrowed("byte_buffer"),
        PropertyValueType::Numbery=> Cow::Borrowed("integer"),
        PropertyValueType::Floaty => Cow::Borrowed("float"),
        PropertyValueType::Booly => Cow::Borrowed("bool"),
        PropertyValueType::SockAddr => Cow::Borrowed("socket_address"),
        PropertyValueType::IpAddr => Cow::Borrowed("ip_address"),
        PropertyValueType::PortNumber => Cow::Borrowed("port_number"),
        PropertyValueType::Path => Cow::Borrowed("filesystem_path"),
        PropertyValueType::Uri => Cow::Borrowed("URI"),
        PropertyValueType::Duration => Cow::Borrowed("time_duration"),
        PropertyValueType::ChildNode => Cow::Borrowed("subnode"),
        PropertyValueType::OsString => Cow::Borrowed("osstring"),
        PropertyValueType::Enummy(v) => {
            let mut s = String::with_capacity(40);
            s.push_str("enum with values:");
            for (_,x) in v {
                s.push(' ');
                s.push_str(x);
            }
            Cow::Owned(s)
        }
    }
} 

pub fn help(mode: HelpMode, reg: &ClassRegistrar, allopts: &HashMap<String, PropertyValueType>) {
    if matches!(mode, HelpMode::Full | HelpMode::Short) {
        crate::version();
        println!("Command line client for WebSockets (RFC 6455), also general socat-like interconnector with web features.");
        println!("Created by Vitaly \"_Vi\" Shukela. Questions and problems: https://github.com/vi/websocat/issues/");
        println!();
        print!(
            r#"Usage:
        websocat ws://URL wss://URL     (simple client)
        websocat -s port                (simple server)
        websocat [OPTIONS] arg1:...  arg2:... (advanced mode)

Some advanced mode examples:
    WebSocket-to-TCP proxy: websocat --binary ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    TCP-to-WebSocket proxy: websocat --binary tcp-l:127.0.0.1:5678 ws://127.0.0.1:8080
    See README and other pages in Github repository for more examples.

Options and flags:
"#
        );
    }

    enum OptionType {
        Core,
        FromClass,
    };
    struct OptionInfo {
        typ: OptionType,
        short: Option<char>,
        help: String,
        arg: String,
    }

    let mut all_long_opts: BTreeMap<String, OptionInfo> = BTreeMap::new();
    let mut printed_entries = 0;

    for (prop, arginfo, help) in crate::CORE_OPTS {
        all_long_opts.insert(
            prop.to_owned(),
            OptionInfo {
                typ: OptionType::Core,
                short: None,
                help: help.to_owned(),
                arg: arginfo.to_owned(),
            },
        );
    }

    for (prop, _) in allopts {
        all_long_opts.insert(
            prop.to_owned(),
            OptionInfo {
                typ: OptionType::FromClass,
                short: None,
                help: "TODO".to_owned(),
                arg: "".to_owned(),
            },
        );
    }
    if matches!(mode, HelpMode::Full | HelpMode::Short) {
        for (short, long) in crate::SHORT_OPTS {
            if let Some(op) = all_long_opts.get_mut(long) {
                op.short = Some(short);
            } else {
                eprintln!(
                    "ERROR: short option `{}` refers non-existant long option `{}`",
                    short, long
                );
            }
        }

        for (opt, info) in all_long_opts {
            if let Some(short) = info.short {
                print!(" -{}, ", short);
            } else {
                print!("     ");
            }
            let opt_n_arg = format!("{} {}", opt, info.arg);
            print!("--{:30}", opt_n_arg);

            let longhelp = info.help.len() > 60;
            if matches!(info.typ, OptionType::Core) && !longhelp {
                println!("{}", info.help);
                continue;
            }

            println!();

            for helpline in textwrap::wrap(&info.help, 100) {
                println!("        {}", helpline.as_ref());
            }
        }
    }

    if matches!(mode, HelpMode::Full ) {
        println!("\nList of all nodes and with their properties:");
    }
    if matches!(mode, HelpMode::Short) {
        println!("\nList of all nodes:");
    }

    // BTreeMap for sorting
    let mut classes : BTreeMap<String, &DNodeClass> = BTreeMap::new();
    for cls in reg.classes() {
        let name = cls.official_name();
        if let HelpMode::SpecificThing(ref x) = mode {
            if ! x.eq_ignore_ascii_case(&name) {
                continue;
            }
        }
        classes.insert(name, cls);
    }
    for (official_name, cls) in classes.iter() {
        println!("  node `{}`", official_name);

        if ! matches!(mode, HelpMode::Short | HelpMode::JustListThings) {
            if let Some(at) = cls.array_type() {
                println!("    accepts array of elements of type {}", format_pvt(&at));
            }
            if let Some(ah) = cls.array_help() {
                for helpline in textwrap::wrap(&ah, 100) {
                    println!("        {}", helpline.as_ref());
                }
            }
            for p in cls.properties() {
                println!("    prop `{}` of type {}", p.name, format_pvt(&p.r#type));
                if let Some(lo) = p.inject_cli_long_option {
                    println!("        Can be set by `--{}`", lo);
                }
                for helpline in textwrap::wrap(&(*p.help)(), 100) {
                    println!("        {}", helpline.as_ref());
                }
            }
            println!("  end of node `{}`", cls.official_name());
        }
        printed_entries+=1;
    }
    drop(classes);

    if matches!(mode, HelpMode::Full | HelpMode::Short) {
        println!("\nList of all macros:");
    }

    let mut macros : BTreeMap<String, &DMacro> = BTreeMap::new();
    for r#macro in reg.macros() {
        let name = r#macro.official_name();
        if let HelpMode::SpecificThing(ref x) = mode {
            if ! x.eq_ignore_ascii_case(&name) {
                continue;
            }
        }
        macros.insert(name, r#macro);
    }
    for (official_name, r#macro) in macros.iter() {
        println!("  macro `{}`", official_name);
        if ! matches!(mode, HelpMode::Short | HelpMode::JustListThings) {
            println!("  end macro `{}`", r#macro.official_name());
        }
        printed_entries+=1;
    }
    drop(macros);

    if matches!(mode, HelpMode::Full) {
        println!("\nUse --help=short to get shorter help message");
    }

    if printed_entries == 0 {
        if let HelpMode::SpecificThing(ref x) = mode {
            println!("Unkonwn --help mode `{}`. \
                      Valid values are short, long (full), manpage, markdown, list, \
                      Or just no value at all. `-?` also implies short mode.", x);
        }
    }
}
