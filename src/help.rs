use super::{Opt,SpecifierClass,StructOpt};

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
    let mut special_A_permit = false;
    for l in BufReader::new(&b[..]).lines() {
        if let Ok(l) = l {
            {
                let lt = l.trim();
                let new_paragraph_start = false || lt.starts_with('-') || l.is_empty();
                if lt.starts_with("--long-help") {
                    special_A_permit = true;
                }
                if l.contains("[A]") {
                    if special_A_permit {
                        special_A_permit = false;
                    } else {
                        do_display = false;
                        if l.trim().starts_with("[A]") {
                            // Also retroactively retract the previous line
                            let nl = lines_to_display.len() - 1;
                            lines_to_display.truncate(nl);
                        }
                    }
                } else if new_paragraph_start {
                    do_display = true;
                };
            }
            let mut additional_line = None;

            if l == "FLAGS:" {
                additional_line = Some("    (some flags are hidden, see --long-help)".to_string());
            };
            if l == "OPTIONS:" {
                additional_line =
                    Some("    (some options are hidden, see --long-help)".to_string());
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
    //let _ = std::io::stdout().write_all(&b);
}

pub fn longhelp() {
    //let q = Opt::from_iter(vec!["-"]);
    let mut a = Opt::clap();

    let _ = a.print_help();

    // TODO: promote first alias to title
    println!(
        r#"
    
Positional arguments to websocat are generally called specifiers.
Specifiers are ways to obtain a connection from some string representation (i.e. address).

Specifiers may be argumentless (like `mirror:`), can accept an argument (which
may be some path or socket address, like `tcp:`), or can accept a subspecifier
(like `reuse:` or `autoreconnect:`).

Here is the full list of specifier classes in this WebSocat build:

"#
    );

    fn help1(sc: &SpecifierClass) {
        let n = sc.get_name().replace("Class", "");
        let prefixes = sc
            .get_prefixes()
            .iter()
            .map(|x| format!("`{}`", x))
            .collect::<Vec<_>>()
            .join(", ");
        println!("### {}\n\n* {}", n, prefixes);

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

    macro_rules! my {
        ($x:expr) => {
            help1(&$x);
        };
    }

    list_of_all_specifier_classes!(my);

    println!(
        r#"
  
  
TODO:
  sctp:
  ssl:

Final example just for fun: wacky mode

    websocat ws-c:ws-l:ws-c:- tcp:127.0.0.1:5678
    
Connect to a websocket using stdin/stdout as a transport,
then accept a websocket connection over the previous websocket used as a transport,
then connect to a websocket using previous step as a transport,
then forward resulting connection to the TCP port.

(Excercise to the reader: manage to make it actually connect to 5678).
"#
    );
}
