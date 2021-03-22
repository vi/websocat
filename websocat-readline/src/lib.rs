use websocat_derive::{WebsocatNode};
use websocat_api::Result;
use websocat_api::sync::*;


#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "readline")]
pub struct Readline {
  
}

impl websocat_api::sync::Node for Readline {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: websocat_api::RunContext,
        _allow_multiconnect: bool,
        mut closure: impl FnMut(Bipipe) -> Result<()> + Send + 'static,
    ) -> Result<()> {
        std::thread::spawn(move || {
            let mut ed : rustyline::Editor<()> = rustyline::Editor::new();

            let p = Bipipe {
                r: Source::Datagrams(Box::new(move || {
                    let l = ed.readline(":% ")?;
                    ed.add_history_entry(&l);
                    Ok(l.into())
                })),
                w: Sink::Datagrams(Box::new(move |buf| {
                    use std::io::Write;
                    std::io::stdout().write_all(&buf[..])?;
                    Ok(())
                })),
                closing_notification: None,
            };
            let _ = closure(p);
        });
        Ok(())
    }
}
