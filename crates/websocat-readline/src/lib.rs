use websocat_derive::{WebsocatNode};
use websocat_api::{Result, tracing};
use websocat_api::sync::{Bipipe, Node, Source, Sink};
use websocat_api::{anyhow};


#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "readline")]
pub struct Readline {
  
}

impl Node for Readline {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: websocat_api::RunContext,
        _allow_multiconnect: bool,
        mut closure: impl FnMut(Bipipe) -> Result<()> + Send + 'static,
    ) -> Result<()> {
        let ed = linefeed::Interface::new("websocat")?;
        //ed.lock_reader().set_catch_signals(true);
        ed.set_prompt("websocat> ")?;
        //ed.set_report_signal(linefeed::terminal::Signal::Interrupt, true);
        //ed.set_ignore_signal(linefeed::terminal::Signal::Interrupt, false);
        //ed.set_ignore_signal(linefeed::terminal::Signal::Interrupt, true);
        let ed = std::sync::Arc::new(ed);
        std::thread::spawn(move || {
            let ed2 = ed.clone();
            let p = Bipipe {
                r: Source::Datagrams(Box::new(move || {
                    match ed.read_line()? {
                        linefeed::ReadResult::Eof => {
                            tracing::info!("EOF");
                            Ok(None)
                        }
                        linefeed::ReadResult::Input(x) => {
                            ed.add_history_unique(x.clone());
                            Ok(Some(x.into()))
                        }
                        linefeed::ReadResult::Signal(e) => {
                            Err(anyhow::anyhow!("Signal arrived: {:?}", e))
                        }
                    }
                })),
                w: Sink::Datagrams(Box::new(move |buf| {
                    writeln!(ed2, "{}", String::from_utf8_lossy(&buf[..]))?;
                    Ok(())
                })),
                closing_notification: None,
            };
            let _ = closure(p);
        });
        Ok(())
    }
}
