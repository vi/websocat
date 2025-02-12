use bytes::BytesMut;

use super::{types::{BufferFlag, BufferFlags}, utils1::IsControlFrame};


/// Assembles datagram from multiple sequention concatenated parts
pub struct Defragmenter {
    /// Presense of this indicates there is some incomplete unsent (or not fully sent) data
    incomplete_outgoing_datagram_buffer: Option<BytesMut>,

    /// `true` means that we have assembled the datagram fully, but failed to deliver it yet.
    incomplete_outgoing_datagram_buffer_complete: bool,
}

pub enum DefragmenterAddChunkResult<'a> {
    DontSendYet,
    Continunous(&'a [u8]),
}

impl Defragmenter {
    pub fn new() -> Defragmenter {
        Defragmenter {
            incomplete_outgoing_datagram_buffer: None,
            incomplete_outgoing_datagram_buffer_complete: false,
        }
    }

    pub fn add_chunk<'a>(
        &'a mut self,
        buf: &'a mut [u8],
        flags: BufferFlags,
    ) -> DefragmenterAddChunkResult<'a> {
        let this = self;

        // control packets are typically for WebSocket things like pings, so let's ignore them
        if flags.is_control() {
            return DefragmenterAddChunkResult::DontSendYet;
        }

        
        if flags.contains(BufferFlag::NonFinalChunk) {
            this.incomplete_outgoing_datagram_buffer
                .get_or_insert_with(Default::default)
                .extend_from_slice(buf);
            return DefragmenterAddChunkResult::DontSendYet;
        }
        let data: &[u8] = if let Some(ref mut x) = this.incomplete_outgoing_datagram_buffer {
            if !this.incomplete_outgoing_datagram_buffer_complete {
                x.extend_from_slice(buf);
                this.incomplete_outgoing_datagram_buffer_complete = true;
            }
            &x[..]
        } else {
            buf
        };
        DefragmenterAddChunkResult::Continunous(data)
    }

    pub fn clear(&mut self) {
        self.incomplete_outgoing_datagram_buffer_complete = false;
        self.incomplete_outgoing_datagram_buffer = None;
    }
}
