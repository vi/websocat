use super::futures::{Future, Stream};
use super::{
    futures, my_copy, ConstructParams, L2rUser, L2rWriter, Options, Peer, PeerConstructor,
    ProgramState, Session, Specifier, Transfer,
};
use spawn_hack;
use std;
use std::cell::RefCell;
use std::rc::Rc;
use tokio_io;

impl Session {
    pub fn run(self) -> Box<Future<Item = (), Error = Box<std::error::Error>>> {
        let once = self.2.one_message;
        let co = my_copy::CopyOptions {
            stop_on_reader_zero_read: true,
            once,
            buffer_size: self.2.buffer_size,
        };
        let f1 = my_copy::copy(self.0.from, self.0.to, co);
        let f2 = my_copy::copy(self.1.from, self.1.to, co);
        // TODO: properly shutdown in unidirectional mode
        let f1 = f1.and_then(|(_, r, w)| {
            info!("Forward finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                info!("Forward shutdown finished");
                std::mem::drop(w);
            })
        });
        let f2 = f2.and_then(|(_, r, w)| {
            info!("Reverse finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                info!("Reverse shutdown finished");
                std::mem::drop(w);
            })
        });

        let (unif, unir, eeof) = (
            self.2.unidirectional,
            self.2.unidirectional_reverse,
            self.2.exit_on_eof,
        );
        type Ret = Box<Future<Item = (), Error = Box<std::error::Error>>>;
        match (unif, unir, eeof) {
            (false, false, false) => Box::new(
                f1.join(f2)
                    .map(|(_, _)| {
                        info!("Finished");
                    })
                    .map_err(|x| Box::new(x) as Box<std::error::Error>),
            ) as Ret,
            (false, false, true) => Box::new(
                f1.select(f2)
                    .map(|(_, _)| {
                        info!("One of directions finished");
                    })
                    .map_err(|(x, _)| Box::new(x) as Box<std::error::Error>),
            ) as Ret,
            (true, false, _) => Box::new({
                ::std::mem::drop(f2);
                f1.map_err(|x| Box::new(x) as Box<std::error::Error>)
            }) as Ret,
            (false, true, _) => Box::new({
                ::std::mem::drop(f1);
                f2.map_err(|x| Box::new(x) as Box<std::error::Error>)
            }) as Ret,
            (true, true, _) => Box::new({
                // Just open connection and close it.
                ::std::mem::drop(f1);
                ::std::mem::drop(f2);
                futures::future::ok(())
            }) as Ret,
        }
    }
    pub fn new(peer1: Peer, peer2: Peer, opts: Rc<Options>) -> Self {
        Session(
            Transfer {
                from: peer1.0,
                to: peer2.1,
            },
            Transfer {
                from: peer2.0,
                to: peer1.1,
            },
            opts,
        )
    }
}

fn l2r_new() -> L2rWriter {
    Rc::new(RefCell::new(Default::default()))
}

pub fn serve<OE>(
    s1: Rc<Specifier>,
    s2: Rc<Specifier>,
    opts: Options,
    onerror: std::rc::Rc<OE>,
) -> impl Future<Item = (), Error = ()>
where
    OE: Fn(Box<std::error::Error>) -> () + 'static,
{
    futures::future::ok(()).and_then(|()| serve_impl(s1, s2, opts, onerror))
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn serve_impl<OE>(
    s1: Rc<Specifier>,
    s2: Rc<Specifier>,
    opts: Options,
    onerror: std::rc::Rc<OE>,
) -> Box<Future<Item = (), Error = ()>>
where
    OE: Fn(Box<std::error::Error>) -> () + 'static,
{
    info!("Serving {:?} to {:?} with {:?}", s1, s2, opts);
    let ps = Rc::new(RefCell::new(ProgramState::default()));

    use PeerConstructor::{Overlay1, OverlayM, ServeMultipleTimes, ServeOnce};

    let e1 = onerror.clone();
    let e2 = onerror.clone();
    let e3 = onerror.clone();

    let opts1 = Rc::new(opts);
    let opts2 = opts1.clone();

    let l2r = l2r_new();

    let cp = Rc::new(RefCell::new(ConstructParams {
        program_options: opts1.clone(),
        global_state: ps.clone(),
        left_to_right: L2rUser::FillIn(l2r.clone()),
    }));

    let mut left = s1.construct(cp.borrow().clone());

    if opts2.oneshot {
        left =
            PeerConstructor::ServeOnce(left.get_only_first_conn(cp.borrow().left_to_right.clone()));
    }

    let max_parallel_conns = opts1.max_parallel_conns;
    let current_parallel_conns = Rc::new(::std::cell::Cell::new(0usize));

    match left {
        ServeMultipleTimes(stream) => {
            let runner = stream
                .map(move |peer1| {
                    let mut cpc = current_parallel_conns.get();
                    let cpc2 = current_parallel_conns.clone();
                    cpc += 1;
                    if let Some(cap) = max_parallel_conns {
                        if cpc > cap {
                            warn!("Dropping connection because of connection cap");
                            return;
                        }
                    }
                    info!("Serving {} ongoing connections", cpc);
                    current_parallel_conns.set(cpc);

                    let opts3 = opts2.clone();
                    let e1_1 = e1.clone();
                    let cp2 = cp.borrow().reply();
                    cp.borrow_mut().reset_l2r();
                    let l2rc = cp2.left_to_right.clone();
                    spawn_hack(
                        s2.construct(cp2)
                            .get_only_first_conn(l2rc)
                            .and_then(move |peer2| {
                                let s = Session::new(peer1, peer2, opts3);
                                s.run()
                            })
                            .map_err(move |e| e1_1(e))
                            .then(move |r| {
                                cpc2.set(cpc2.get() - 1);
                                futures::future::result(r)
                            }),
                    )
                })
                .for_each(|()| futures::future::ok(()));
            Box::new(runner.map_err(move |e| e2(e))) as Box<Future<Item = (), Error = ()>>
        }
        OverlayM(stream, mapper) => {
            let runner = stream
                .map(move |peer1_| {
                    debug!("Underlying connection established");

                    let mut cpc = current_parallel_conns.get();
                    let cpc2 = current_parallel_conns.clone();
                    cpc += 1;
                    if let Some(cap) = max_parallel_conns {
                        if cpc > cap {
                            warn!("Dropping connection because of connection cap");
                            return;
                        }
                    }
                    info!("Serving {} ongoing connections", cpc);
                    current_parallel_conns.set(cpc);

                    let cp_ = cp.borrow().deep_clone();
                    cp.borrow_mut().reset_l2r();
                    let opts3 = opts2.clone();
                    let e1_1 = e1.clone();
                    let s2 = s2.clone();
                    let l2rc = cp_.left_to_right.clone();
                    spawn_hack(
                        mapper(peer1_, l2rc)
                            .and_then(move |peer1| {
                                let cp2 = cp_.reply();
                                let l2rc = cp2.left_to_right.clone();
                                s2.construct(cp2)
                                    .get_only_first_conn(l2rc)
                                    .and_then(move |peer2| {
                                        let s = Session::new(peer1, peer2, opts3);
                                        s.run()
                                    })
                            })
                            .map_err(move |e| e1_1(e))
                            .then(move |r| {
                                cpc2.set(cpc2.get() - 1);
                                futures::future::result(r)
                            }),
                    )
                })
                .for_each(|()| futures::future::ok(()));
            Box::new(runner.map_err(move |e| e2(e))) as Box<Future<Item = (), Error = ()>>
        }
        ServeOnce(peer1c) => {
            let runner = peer1c.and_then(move |peer1| {
                let cp2 = cp.borrow().reply();
                let l2rc = cp2.left_to_right.clone();
                let right = s2.construct(cp2);
                let fut = right.get_only_first_conn(l2rc);
                fut.and_then(move |peer2| {
                    let s = Session::new(peer1, peer2, opts2);
                    s.run().map(|()| {
                        ::std::mem::drop(ps)
                        // otherwise ps will be dropped sooner
                        // and stdin/stdout may become blocking sooner
                    })
                })
            });
            Box::new(runner.map_err(move |e| e3(e))) as Box<Future<Item = (), Error = ()>>
        }
        Overlay1(peer1c, mapper) => {
            let runner = peer1c.and_then(move |peer1_| {
                let l2rc = cp.borrow().left_to_right.clone();
                debug!("Underlying connection established");
                mapper(peer1_, l2rc).and_then(move |peer1| {
                    let cp2 = cp.borrow().reply();
                    let l2rc = cp2.left_to_right.clone();
                    let right = s2.construct(cp2);
                    let fut = right.get_only_first_conn(l2rc);
                    fut.and_then(move |peer2| {
                        let s = Session::new(peer1, peer2, opts2);
                        s.run().map(|()| {
                            ::std::mem::drop(ps)
                            // otherwise ps will be dropped sooner
                            // and stdin/stdout may become blocking sooner
                        })
                    })
                })
            });
            Box::new(runner.map_err(move |e| e3(e))) as Box<Future<Item = (), Error = ()>>
        }
    }
}
