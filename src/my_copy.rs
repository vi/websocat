use std::io;

use futures::{Future, Poll};

use {AsyncRead, AsyncWrite};

/// A future which will copy all data from a reader into a writer.
/// A modified version of tokio_io::copy::Copy.
///
/// Created by the [`copy`] function, this future will resolve to the number of
/// bytes copied or an error if one happens.
///
/// [`copy`]: fn.copy.html
#[derive(Debug)]
pub struct Copy<R, W> {
    reader: Option<R>,
    read_done: bool,
    writer: Option<W>,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Box<[u8]>,
    stop_on_reader_zero_read: bool,
}

/// Creates a future which represents copying all the bytes from one object to
/// another.
///
/// The returned future will copy all the bytes read from `reader` into the
/// `writer` specified. This future will only complete once the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned and the `reader` and `writer` are
/// consumed. On error the error is returned and the I/O objects are consumed as
/// well.
///
/// Unlike original tokio_io::copy::copy, it does not always stop on zero length reads
/// , handles BrokenPipe error kind as EOF and flushes after every write
pub fn copy<R, W>(reader: R, writer: W, stop_on_reader_zero_read: bool) -> Copy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    Copy {
        reader: Some(reader),
        read_done: false,
        writer: Some(writer),
        amt: 0,
        pos: 0,
        cap: 0,
        // TODO - de-hardcode buffer size
        buf: Box::new([0; 65536]),
        stop_on_reader_zero_read,
    }
}

impl<R, W> Future for Copy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    type Item = (u64, R, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(u64, R, W), io::Error> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            trace!("poll");
            if self.pos == self.cap && !self.read_done {
                let reader = self.reader.as_mut().unwrap();
                let rr = reader.read(&mut self.buf);
                if let Err(ref e) = rr {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        debug!("BrokenPipe: read_done");
                        self.read_done = true;
                        continue;
                    }
                }
                let n = try_nb!(rr);
                trace!("read {}", n);
                if n == 0 {
                    debug!("zero len");
                    if self.stop_on_reader_zero_read {
                        debug!("read_done");
                        self.read_done = true;
                    }
                    continue;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while self.pos < self.cap {
                let writer = self.writer.as_mut().unwrap();
                let i = try_nb!(writer.write(&self.buf[self.pos..self.cap]));
                if i == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write zero byte into writer",
                    ));
                } else {
                    trace!("write {}", i);
                    self.pos += i;
                    self.amt += i as u64;
                }
                try_nb!(writer.flush());
            }

            // If we've written al the data and we've seen EOF, flush out the
            // data and finish the transfer.
            // done with the entire transfer.
            if self.pos == self.cap && self.read_done {
                try_nb!(self.writer.as_mut().unwrap().flush());
                let reader = self.reader.take().unwrap();
                let writer = self.writer.take().unwrap();
                debug!("done");
                return Ok((self.amt, reader, writer).into());
            }
        }
    }
}
