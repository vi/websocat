use super::futures::{Future, Stream};
use super::{
    futures, my_copy, ConstructParams, L2rUser, L2rWriter, Options, Peer, PeerConstructor,
    ProgramState, Session, Specifier, Transfer,
};
use crate::spawn_hack;
use std;
use std::cell::RefCell;
use std::rc::Rc;
use tokio_io;

impl Session {
    pub fn run(self) -> Box<dyn Future<Item = (), Error = Box<dyn std::error::Error>>> {
        let once = self.opts.one_message;
        let mut co1 = my_copy::CopyOptions {
            stop_on_reader_zero_read: !self.opts.no_exit_on_zeromsg,
            once,
            buffer_size: self.opts.buffer_size,
            skip: false,
            max_ops: self.opts.max_messages,
        };
        let mut co2 = co1.clone();
        co2.max_ops = self.opts.max_messages_rev;
        if self.opts.unidirectional {
            co2.skip=true;
        }
        if self.opts.unidirectional_reverse {
            co1.skip=true;
        }
        let f1 = my_copy::copy(self.t1.from, self.t1.to, co1, self.opts.preamble.clone());
        let f2 = my_copy::copy(self.t2.from, self.t2.to, co2, self.opts.preamble_reverse.clone());

        let f1 = f1.and_then(|(_, r, w)| {
            info!("Forward finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                debug!("Forward shutdown finished");
                std::mem::drop(w);
            })
        });
        let f2 = f2.and_then(|(_, r, w)| {
            info!("Reverse finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                debug!("Reverse shutdown finished");
                std::mem::drop(w);
            })
        });

        type Ret = Box<dyn Future<Item = (), Error = Box<dyn std::error::Error>>>;
        let tmp = if !self.opts.exit_on_eof {
            Box::new(
                f1.join(f2)
                    .map(|(_, _)| {
                        info!("Both directions finished");
                    })
                    .map_err(|x| Box::new(x) as Box<dyn std::error::Error>),
            ) as Ret
        } else {
            Box::new(
                f1.select(f2)
                    .map(|(_, _)| {
                        info!("One of directions finished");
                    })
                    .map_err(|(x, _)| Box::new(x) as Box<dyn std::error::Error>),
            ) as Ret
        };
        // tmp is now everything except of HUP handling
        if self.hup1.is_none() && self.hup2.is_none() {
            tmp // no need for complications
        } else {
            let mut s = futures::stream::futures_unordered::FuturesUnordered::new();
            s.push(tmp);
            if let Some(hup) = self.hup1 {
                s.push(hup);
            }
            if let Some(hup) = self.hup2 {
                s.push(hup);
            }
            Box::new(
                s.into_future()
                .map(|(x, _)|x.unwrap())
                .map_err(|(e,_)|e)
            ) as Ret
        }
    }
    pub fn new(peer1: Peer, peer2: Peer, opts: Rc<Options>) -> Self {
        Session{
            t1: Transfer {
                from: peer1.0,
                to: peer2.1,
            },
            t2: Transfer {
                from: peer2.0,
                to: peer1.1,
            },
            opts,
            hup1: peer1.2,
            hup2: peer2.2,
        }
    }
}

fn l2r_new() -> L2rWriter {
    Rc::new(RefCell::new(Default::default()))
}

pub fn serve<OE>(
    s1: Rc<dyn Specifier>,
    s2: Rc<dyn Specifier>,
    opts: Options,
    onerror: std::rc::Rc<OE>,
) -> impl Future<Item = (), Error = ()>
where
    OE: Fn(Box<dyn std::error::Error>) -> () + 'static,
{
    futures::future::ok(()).and_then(|()| serve_impl(s1, s2, opts, onerror))
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
fn serve_impl<OE>(
    s1: Rc<dyn Specifier>,
    s2: Rc<dyn Specifier>,
    opts: Options,
    onerror: std::rc::Rc<OE>,
) -> Box<dyn Future<Item = (), Error = ()>>
where
    OE: Fn(Box<dyn std::error::Error>) -> () + 'static,
{
    debug!("Serving {:?} to {:?} with {:?}", s1, s2, opts);
    let ps = Rc::new(RefCell::new(ProgramState::default()));

    use crate::PeerConstructor::{Overlay1, OverlayM, ServeMultipleTimes, ServeOnce};

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


    #[cfg(feature = "prometheus_peer")]
    {
        if let Some(psa) = opts1.prometheus {
            let _ /*: crate::prometheus_peer::GlobalState*/ = cp.as_ref().borrow().global(crate::prometheus_peer::new_global_stats);
            if let Err(e) = crate::prometheus_peer::serve(psa) {
                error!("Error listening Prometheus exposer socket: {}", e);
            }
        }
    }

    let mut left = s1.construct(cp.borrow().clone());

    if opts2.oneshot {
        left =
            PeerConstructor::ServeOnce(left.get_only_first_conn(cp.borrow().left_to_right.clone()));
    }

    let max_parallel_conns = opts1.max_parallel_conns;
    let current_parallel_conns = Rc::new(::std::cell::Cell::new(0usize));

    match left {
        PeerConstructor::Error(e) => {
            e1(e);
            Box::new(futures::future::ok(())) as Box<dyn Future<Item = (), Error = ()>>
        },
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
            Box::new(runner.map_err(move |e| e2(e))) as Box<dyn Future<Item = (), Error = ()>>
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
            Box::new(runner.map_err(move |e| e2(e))) as Box<dyn Future<Item = (), Error = ()>>
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
            Box::new(runner.map_err(move |e| e3(e))) as Box<dyn Future<Item = (), Error = ()>>
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
            Box::new(runner.map_err(move |e| e3(e))) as Box<dyn Future<Item = (), Error = ()>>
        }
    }
}
