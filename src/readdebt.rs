use std;

#[derive(Debug, Clone, Copy)]
pub enum DebtHandling {
    Silent,
    Warn,
    DropMessage,
}

pub enum ZeroMessagesHandling {
    Drop,
    Deliver,
}

pub enum ProcessMessageResult {
    Return(std::result::Result<usize, std::io::Error>),
    Recurse,
}

/// A `Read` utility to deal with partial reads
pub struct ReadDebt(pub Option<Vec<u8>>, pub DebtHandling, pub ZeroMessagesHandling);
impl ReadDebt {
    pub fn process_message(&mut self, buf: &mut [u8], buf_in: &[u8]) -> ProcessMessageResult {
        assert_eq!(self.0, None);
        let mut l = buf_in.len();
        if l > buf.len() {
            match self.1 {
                DebtHandling::Silent => (),
                DebtHandling::Warn => {
                    warn!("Incoming message too long ({} > {}): splitting it to parts.\nUse -B option to increase buffer size or -S option to drop messages instead of splitting.", l, buf.len());
                }
                DebtHandling::DropMessage => {
                    error!("Dropping too large message ({} > {}). Use -B option to increase buffer size.", l, buf.len());
                    return ProcessMessageResult::Recurse;
                }
            }
            l = buf.len();
        }
        buf[..l].copy_from_slice(&buf_in[..l]);

        if l < buf_in.len() {
            self.0 = Some(buf_in[l..].to_vec());
        }

        debug!("Fulfilling the debt of {} bytes", l);
        if l == 0 {
            match self.2 {
                ZeroMessagesHandling::Deliver => (),
                ZeroMessagesHandling::Drop => {
                    info!("Dropping incoming zero-length message");
                    return ProcessMessageResult::Recurse;
                }
            }
        }
        ProcessMessageResult::Return(Ok(l))
    }
    pub fn check_debt(
        &mut self,
        buf: &mut [u8],
    ) -> Option<std::result::Result<usize, std::io::Error>> {
        if let Some(debt) = self.0.take() {
            match self.process_message(buf, debt.as_slice()) {
                ProcessMessageResult::Return(x) => Some(x),
                ProcessMessageResult::Recurse => unreachable!(),
            }
        } else {
            None
        }
    }
}
