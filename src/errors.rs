use std::error::Error;
use std::fmt;
use std::fmt::write;

#[derive(Debug)]
pub enum WireSyncError {
    ServerDuplicatedError((String,i32)),
}

impl fmt::Display for WireSyncError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::WireSyncError::*;
        match self {
            ServerDuplicatedError(t) => write!(f, "{}:{} already exists.",t.0, t.1),
        }
    }
}

impl Error for WireSyncError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}