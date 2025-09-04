use std::{
    ffi::OsString,
    io::{Cursor, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::scenario_executor::exit_code::ExitCodeTracker;



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
impl Default for SharedCursor {
    fn default() -> Self {
        Self::new()
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
    let exit_code = ExitCodeTracker::new();

    let ret = crate::websocat_main(argv, stderr.clone(), time_base, false, registry, exit_code).await;

    if let Err(ref e) = ret {
        std::io::stderr().write_all(&stderr.content()).unwrap();
        eprintln!("{}", e);
    }

    assert!(ret.is_ok());
}

pub async fn test_two_websocats(s1: &str, s2: &str, wait_ms: u64) {
    let mut argv1: Vec<OsString> = vec!["websocat".into()];
    argv1.extend(shlex::split(s1).unwrap().into_iter().map(|x| x.into()));
    let mut argv2: Vec<OsString> = vec!["websocat".into()];
    argv2.extend(shlex::split(s2).unwrap().into_iter().map(|x| x.into()));

    let time_base = tokio::time::Instant::now();
    let stderr1 = SharedCursor::new();
    let stderr2 = SharedCursor::new();
    let registry = super::scenario_executor::types::Registry::default();
    let exit_code = ExitCodeTracker::new();

    //dbg!(&argv1, &argv2);

    // Websocat instances can communicate using e.g. `registry-stream-listen:` and `registry-stream-connect:` specifiers.
    let wsc1 = crate::websocat_main(argv1, stderr1.clone(), time_base, false, registry.clone(), exit_code.clone());
    let wsc2 = crate::websocat_main(argv2, stderr2.clone(), time_base, false, registry.clone(), exit_code.clone());

    let h1 = tokio::spawn(wsc1);

    tokio::time::sleep(Duration::from_millis(wait_ms)).await;

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

pub async fn test_three_websocats(s1: &str, s2: &str, s3: &str, wait1_ms: u64, wait2_ms: u64) {
    //tracing_subscriber::fmt::init();
    let mut argv1: Vec<OsString> = vec!["websocat".into()];
    argv1.extend(shlex::split(s1).unwrap().into_iter().map(|x| x.into()));
    let mut argv2: Vec<OsString> = vec!["websocat".into()];
    argv2.extend(shlex::split(s2).unwrap().into_iter().map(|x| x.into()));
    let mut argv3: Vec<OsString> = vec!["websocat".into()];
    argv3.extend(shlex::split(s3).unwrap().into_iter().map(|x| x.into()));

    let time_base = tokio::time::Instant::now();
    let stderr1 = SharedCursor::new();
    let stderr2 = SharedCursor::new();
    let stderr3 = SharedCursor::new();
    let registry = super::scenario_executor::types::Registry::default();
    let exit_code = ExitCodeTracker::new();

    //dbg!(&argv1, &argv2);

    // Websocat instances can communicate using e.g. `registry-stream-listen:` and `registry-stream-connect:` specifiers.
    let wsc1 = crate::websocat_main(argv1, stderr1.clone(), time_base, false, registry.clone(), exit_code.clone());
    let wsc2 = crate::websocat_main(argv2, stderr2.clone(), time_base, false, registry.clone(), exit_code.clone());
    let wsc3 = crate::websocat_main(argv3, stderr3.clone(), time_base, false, registry.clone(), exit_code.clone());

    let h1 = tokio::spawn(wsc1);

    tokio::time::sleep(Duration::from_millis(wait1_ms)).await;

    let h2 = tokio::spawn(wsc2);

    tokio::time::sleep(Duration::from_millis(wait2_ms)).await;

    let ret3 = wsc3.await;
    let ret1 = h1.await.unwrap();
    let ret2 = h2.await.unwrap();

    if let Err(ref e) = ret1 {
        std::io::stderr().write_all(&stderr1.content()).unwrap();
        eprintln!("{}", e);
    }
    if let Err(ref e) = ret2 {
        std::io::stderr().write_all(&stderr2.content()).unwrap();
        eprintln!("{}", e);
    }
    if let Err(ref e) = ret3 {
        std::io::stderr().write_all(&stderr3.content()).unwrap();
        eprintln!("{}", e);
    }

    assert!(ret1.is_ok());
    assert!(ret2.is_ok());
    assert!(ret3.is_ok());
}

#[macro_export]
macro_rules! t {
    ($n:ident, $x:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_websocat($x).await;
        }
    };
}
#[macro_export]
macro_rules! t_p {
    ($n:ident, $x:literal $(,)?) => {
        #[tokio::test(start_paused = true)]
        async fn $n() {
            websocat::test_utils::test_websocat($x).await;
        }
    };
}

#[macro_export]
macro_rules! t2 {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_two_websocats($x, $y, 0).await;
        }
    };
}

#[macro_export]
macro_rules! t2w {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_two_websocats($x, $y, 50).await;
        }
    };
}

#[macro_export]
macro_rules! t3w {
    ($n:ident, $x:literal, $y:literal, $z:literal $(,)?) => {
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_three_websocats($x, $y, $z, 50, 10).await;
        }
    };
}

#[macro_export]
macro_rules! t3w_p {
    ($n:ident, $x:literal, $y:literal, $z:literal $(,)?) => {
        #[tokio::test(start_paused = true)]
        async fn $n() {
            websocat::test_utils::test_three_websocats($x, $y, $z, 50, 10).await;
        }
    };
}

#[macro_export]
macro_rules! t_unix {
    ($n:ident, $x:literal $(,)?) => {
        #[cfg(unix)]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_websocat($x).await;
        }
    };
}

#[macro_export]
macro_rules! t2w_unix {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[cfg(unix)]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_two_websocats($x, $y, 50).await;
        }
    };
}

#[macro_export]
macro_rules! t_online {
    ($n:ident, $x:literal $(,)?) => {
        #[cfg(feature = "online_tests")]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_websocat($x).await;
        }
    };
}

#[macro_export]
macro_rules! t2w_online {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[cfg(feature = "online_tests")]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_two_websocats($x, $y, 50).await;
        }
    };
}

#[macro_export]
macro_rules! t_linux {
    ($n:ident, $x:literal $(,)?) => {
        #[cfg(target_os = "linux")]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_websocat($x).await;
        }
    };
}
#[macro_export]
macro_rules! t2w_linux {
    ($n:ident, $x:literal, $y:literal $(,)?) => {
        #[cfg(target_os = "linux")]
        #[tokio::test]
        async fn $n() {
            websocat::test_utils::test_two_websocats($x, $y, 50).await;
        }
    };
}
