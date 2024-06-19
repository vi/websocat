use crate::scenario_executor::types::{
    DatagramRead, DatagramWrite, StreamRead, StreamSocket, StreamWrite,
};

use super::types::DatagramSocket;

impl std::fmt::Debug for StreamSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SS(")?;
        if let Some(ref r) = self.read {
            r.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(ref w) = self.write {
            w.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(_) = self.close {
            write!(f, "H")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl std::fmt::Debug for StreamRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SR")?;
        if !self.prefix.is_empty() {
            write!(f, "{{{}}}", self.prefix.len())?;
        }
        write!(f, "@{:p}", self.reader)
    }
}
impl std::fmt::Debug for StreamWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SW@{:p}", self.writer)
    }
}

impl std::fmt::Debug for DatagramRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DR@{:p}", self.src)
    }
}

impl std::fmt::Debug for DatagramWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DW@{:p}", self.snk)
    }
}

impl std::fmt::Debug for DatagramSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DS(")?;
        if let Some(ref r) = self.read {
            r.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(ref w) = self.write {
            w.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(_) = self.close {
            write!(f, "H")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

pub struct PtrDbg<T: std::fmt::Pointer>(pub T);
impl<T: std::fmt::Pointer> std::fmt::Debug for PtrDbg<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Pointer::fmt(&self.0, f)
    }
}
