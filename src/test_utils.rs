use std::{ffi::OsString, io::Cursor, sync::{Arc, Mutex}};


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
    argv.extend(shlex::split(s).unwrap().into_iter().map(|x|x.into()));
    
    let time_base = tokio::time::Instant::now();
    let stderr = SharedCursor::new();

    let ret = crate::websocat_main(argv, stderr.clone(), time_base).await;
    assert!(ret.is_ok());
    //std::io::stderr().write_all(&stderr.content()).unwrap();
}
