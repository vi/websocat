use std::{
    ffi::OsString,
    io::{Cursor, Write},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct SharedCursor(Arc<Mutex<Cursor<Vec<u8>>>>);
impl std::io::Write for SharedCursor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
impl SharedCursor {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Cursor::new(vec![]))))
    }
    pub fn content(&self) -> Vec<u8> {
        self.0.lock().unwrap().get_ref().clone()
    }
}

pub async fn test_websocat(s: &str) {
    let mut argv: Vec<OsString> = vec!["websocat".into()];
    argv.extend(shlex::split(s).unwrap().into_iter().map(|x| x.into()));

    let time_base = tokio::time::Instant::now();
    let stderr = SharedCursor::new();
    let registry = super::scenario_executor::types::Registry::default();

    let ret = crate::websocat_main(argv, stderr.clone(), time_base, false, registry).await;

    if let Err(ref e) = ret {
        std::io::stderr().write_all(&stderr.content()).unwrap();
        eprintln!("{}", e);
    }

    assert!(ret.is_ok());
}

pub async fn test_two_websocats(s1: &str, s2: &str) {
    let mut argv1: Vec<OsString> = vec!["websocat".into()];
    argv1.extend(shlex::split(s1).unwrap().into_iter().map(|x| x.into()));
    let mut argv2: Vec<OsString> = vec!["websocat".into()];
    argv2.extend(shlex::split(s2).unwrap().into_iter().map(|x| x.into()));

    let time_base = tokio::time::Instant::now();
    let stderr1 = SharedCursor::new();
    let stderr2 = SharedCursor::new();
    let registry = super::scenario_executor::types::Registry::default();

    //dbg!(&argv1, &argv2);

    // Websocat instances can communicate using e.g. `registry-stream-listen:` and `registry-stream-connect:` specifiers.
    let wsc1 = crate::websocat_main(argv1, stderr1.clone(), time_base, false, registry.clone());
    let wsc2 = crate::websocat_main(argv2, stderr2.clone(), time_base, false, registry.clone());

    let h1 = tokio::spawn(wsc1);

    let ret2 = wsc2.await;
    let ret1 = h1.await.unwrap();

    if let Err(ref e) = ret1 {
        std::io::stderr().write_all(&stderr1.content()).unwrap();
        eprintln!("{}", e);
    }
    if let Err(ref e) = ret2 {
        std::io::stderr().write_all(&stderr2.content()).unwrap();
        eprintln!("{}", e);
    }

    assert!(ret1.is_ok());
    assert!(ret2.is_ok());
}
