use super::{Opt, SpecifierClass, StructOpt};

fn spechelp(sc: &dyn SpecifierClass, overlays: bool, advanced: bool) {
    if !advanced && sc.help().contains("[A]") {
        return;
    }
    if overlays ^ sc.is_overlay() {
        return;
    }

    let first_prefix = sc.get_prefixes()[0];

    let mut first_help_line = None;
    for l in sc.help().lines() {
        if !l.trim().is_empty() {
            first_help_line = Some(l);
            break;
        }
    }
    if let Some(fhl) = first_help_line {
        println!("\t{:16}\t{}", first_prefix, fhl);
    }
}

// https://github.com/rust-lang/rust/issues/51942
#[cfg_attr(feature = "cargo-clippy", allow(nonminimal_bool))]
pub fn shorthelp() {
    //use std::io::Write;
    use std::io::{BufRead, BufReader};
    let mut b = vec![];
    if Opt::clap().write_help(&mut b).is_err() {
        eprintln!("Error displaying the help message");
    }
    let mut lines_to_display = vec![];
    let mut do_display = true;
    #[allow(non_snake_case)]
    for l in BufReader::new(&b[..]).lines() {
        if let Ok(l) = l {
            {
                let lt = l.trim();
                let new_paragraph_start = false || lt.starts_with('-') || l.is_empty();
                if lt.starts_with("--help") {
                    // Allowed to output [A] regardless
                    do_display = true;
                } else if l.contains("[A]") {
                    do_display = false;
                    if l.trim().starts_with("[A]") {
                        // Also retroactively retract the previous line
                        let nl = lines_to_display.len() - 1;
                        lines_to_display.truncate(nl);
                    }
                } else if new_paragraph_start {
                    do_display = true;
                };
            }
            let mut additional_line = None;

            if l == "FLAGS:" {
                additional_line = Some("    (some flags are hidden, see --help=long)".to_string());
            };
            if l == "OPTIONS:" {
                additional_line =
                    Some("    (some options are hidden, see --help=long)".to_string());
            };

            if do_display {
                lines_to_display.push(l);
                if let Some(x) = additional_line {
                    lines_to_display.push(x);
                }
            };
        }
    }
    for l in lines_to_display {
        println!("{}", l);
    }

    println!("\nPartial list of address types:");

    macro_rules! my {
        ($x:expr) => {
            spechelp(&$x, false, false);
        };
    }
    list_of_all_specifier_classes!(my);

    println!("Partial list of overlays:");

    macro_rules! my {
        ($x:expr) => {
            spechelp(&$x, true, false);
        };
    }
    list_of_all_specifier_classes!(my);

    println!("See more address types with the --help=long option.");
    println!("See short examples and --dump-spec names for most address types and overlays with --help=doc option");
}

pub fn longhelp() {
    //let q = Opt::from_iter(vec!["-"]);
    let mut a = Opt::clap();

    let _ = a.print_help();

    println!("\n\nFull list of address types:");

    macro_rules! my {
        ($x:expr) => {
            spechelp(&$x, false, true);
        };
    }
    list_of_all_specifier_classes!(my);

    println!("Full list of overlays:");

    macro_rules! my {
        ($x:expr) => {
            spechelp(&$x, true, true);
        };
    }
    list_of_all_specifier_classes!(my);
}

fn specdoc(sc: &dyn SpecifierClass, overlays: bool) {
    if sc.is_overlay() ^ overlays {
        return;
    }

    let first_prefix = sc.get_prefixes()[0];
    let spec_name = sc.get_name().replace("Class", "");

    let other_prefixes = sc.get_prefixes()[1..]
        .iter()
        .map(|x| format!("`{}`", x))
        .collect::<Vec<_>>()
        .join(", ");

    println!(r#"### `{}`"#, first_prefix);
    println!();
    if !other_prefixes.is_empty() {
        println!("Aliases: {}  ", other_prefixes);
    }
    println!("Internal name for --dump-spec: {}", spec_name);
    println!();

    let help = 
        sc
        .help()
        //.lines()
        //.map(|x|format!("    {}",x))
        //.collect::<Vec<_>>()
        //.join("\n")
        ;
    println!("{}\n", help);
}

pub fn dochelp() {
    println!(r#"
# Websocat Reference (in progress)

Websocat has many command-line options and special format for positional arguments.

There are three main modes of websocat invocation:

* Simple client mode: `websocat wss://your.server/url`
* Simple server mode: `websocat -s 127.0.0.1:8080`
* Advanced [socat][1]-like mode: `websocat -t ws-l:127.0.0.1:8080 mirror:`

Ultimately in any of those modes websocat creates two connections and exchanges data between them.
If one of the connections is bytestream-oriented (for example the terminal stdin/stdout or a TCP connection), but the other is message-oriented (for example, a WebSocket or UDP) then websocat operates in lines: each line correspond to a message. Details of this are configurable by various options.

`ws-l:` or `mirror:` above are examples of address types. With the exception of special cases like WebSocket URL `ws://1.2.3.4/` or stdio `-`, websocat's positional argument is defined by this rule:

```
<specifier> ::= ( <overlay> ":" )* <addrtype> ":" [address]
```

Some address types may be "aliases" to other address types or combinations of overlays and address types.

[1]:http://www.dest-unreach.org/socat/doc/socat.html

# `--help=long`

"Advanced" options and flags are denoted by `[A]` marker.


```
"#);

    let mut a = Opt::clap();

    let _ = a.print_help();

    println!(
        r#"

```

# Full list of address types

"Advanced" address types are denoted by `[A]` marker.

"#
    );

    macro_rules! my {
        ($x:expr) => {
            specdoc(&$x, false);
        };
    }
    list_of_all_specifier_classes!(my);

    println!(
        r#"

# Full list of overlays

"Advanced" overlays denoted by `[A]` marker.

"#
    );

    macro_rules! my {
        ($x:expr) => {
            specdoc(&$x, true);
        };
    }
    list_of_all_specifier_classes!(my);

    println!(
        r#"
  
### Address types or specifiers to be implemented later:

`sctp:`, `speedlimit:`, `quic:`

### Final example

Final example just for fun: wacky mode

    websocat ws-c:ws-l:ws-c:- tcp:127.0.0.1:5678
    
Connect to a websocket using stdin/stdout as a transport,
then accept a websocket connection over the previous websocket used as a transport,
then connect to a websocket using previous step as a transport,
then forward resulting connection to the TCP port.

(Exercise to the reader: manage to make it actually connect to 5678).
"#
    );
}
