use std::{
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    pin::Pin,
    time::SystemTime,
};

use rand::Rng;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::{
    fs::OpenOptions,
    io::{AsyncRead, AsyncWrite},
};
use tracing::{debug, debug_span, warn, Instrument};

use crate::scenario_executor::{
    scenario::callback_and_continue,
    types::{Handle, StreamRead, StreamSocket, StreamWrite},
    utils1::TaskHandleExt2,
};

use super::{scenario::ScenarioAccess, types::Task, utils1::RhResult};

//@ Open specifid file and read/write it.
fn file_socket(
    ctx: NativeCallContext,
    opts: Dynamic,
    path: OsString,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("file_socket");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Open specified file for writing, not reading
        #[serde(default)]
        write: bool,

        //@ Open specified file for appending, not reading
        #[serde(default)]
        append: bool,

        //@ Do not overwrite existing files, instead use modified randomized name.
        //@ Only relevant for `write` mode.
        #[serde(default)]
        no_overwrite: bool,

        //@ Do not overwrite existing files, instead use modified randomized name.
        //@ Only relevant for `write` mode.
        #[serde(default)]
        auto_rename: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let mut oo = OpenOptions::new();

    if opts.append {
        oo.append(true);
        oo.truncate(false);
        oo.create(true);
    } else if opts.write {
        if opts.no_overwrite || opts.auto_rename {
            oo.write(true);
            oo.truncate(false);
            oo.create_new(true);
        } else {
            oo.write(true);
            oo.truncate(true);
            oo.create(true);
        }
    } else {
        oo.read(true);
    }

    debug!(parent: &span, "options parsed");

    Ok(async move {
        let orig_path = path;
        let mut modified_path_buf: PathBuf;
        let mut attempt_counter = 0;
        let mut effective_path: &Path = Path::new(&orig_path);

        let f = loop {
            debug!("trying to open file {effective_path:?}");
            match oo.open(effective_path).await {
                Ok(f) => break f,
                Err(e) if e.kind() == ErrorKind::AlreadyExists && opts.auto_rename => {
                    if attempt_counter > 10 {
                        warn!("Failed to open unique file in 10 attempts, aborting");
                        anyhow::bail!("Cannot open the file");
                    }
                    modified_path_buf = orig_path.clone().into();

                    let unix_time = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    let fnm = modified_path_buf.file_stem().unwrap_or_default();
                    let fne = modified_path_buf.extension().unwrap_or_default();

                    let mut newname: OsString = fnm.to_owned();
                    newname.push(format!(".{unix_time}"));

                    if attempt_counter > 0 {
                        let q: u16 = the_scenario.prng.lock().unwrap().random();
                        newname.push(format!(".{q}"));
                    }
                    if !fne.is_empty() {
                        newname.push(".");
                        newname.push(fne);
                    }

                    modified_path_buf.set_file_name(newname);

                    attempt_counter += 1;
                    effective_path = &modified_path_buf;

                    continue;
                }
                Err(e) => {
                    warn!("Failed to open file `{effective_path:?}`: {e}");
                    return Err(e.into());
                }
            }
        };

        debug!("file opened");

        #[allow(unused_assignments)]
        let mut fd = None;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            fd = Some(
                // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                unsafe { super::types::SocketFd::new(f.as_raw_fd()) },
            );
        }

        let (r, w): (
            Pin<Box<dyn AsyncRead + Send + 'static>>,
            Pin<Box<dyn AsyncWrite + Send + 'static>>,
        ) = if opts.append || opts.write {
            (Box::pin(tokio::io::empty()), Box::pin(f))
        } else {
            (Box::pin(f), Box::pin(tokio::io::empty()))
        };

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
            fd,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("file_socket", file_socket);
}
