//! What can go wrong reading or writing the OCAD format.

use thiserror::Error;

/// A shorthand for results whose error is the OCAD format's [`Error`](enum@Error).
pub type Result<T> = std::result::Result<T, Error>;

/// Why a map could not be read from, or written to, the OCAD format.
#[derive(Debug, Error)]
pub enum Error {
    /// Reading or writing the file itself.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A binary structure in the file could not be read or written.
    #[error(transparent)]
    Binary(#[from] binrw::Error),
    /// The file does not start with the OCAD mark.
    #[error("not an OCAD file")]
    NotOcad,
    /// The file is an OCAD file of a version this crate does not read.
    #[error("OCAD version {0} is not supported; versions 12 and 2018 are")]
    UnsupportedVersion(u16),
    /// The file is an OCAD file of another kind than a map: a course setting
    /// project (1) or a file saved in OCAD Server (8).
    #[error("OCAD file type {0} is not supported; only map files are")]
    NotAMap(u8),
    /// A file position points outside the file.
    #[error("file position {position} is outside the file of {len} bytes")]
    PositionOutOfBounds {
        /// The position the file names.
        position: u64,
        /// The file's length in bytes.
        len: usize,
    },
    /// A chain of index blocks loops back on itself.
    #[error("the index block at {0} is part of a loop")]
    IndexLoop(u32),
    /// The map is too large to be written in this format.
    #[error("the map holds more {0} than the format can store")]
    TooMany(&'static str),
    /// A map coordinate is outside the range the format can store.
    #[error("a map coordinate is outside the range the format can store")]
    CoordinateOutOfRange,
    /// The file describes a map that breaks one of the map's own rules, such
    /// as a combined symbol that contains itself.
    #[error(transparent)]
    Domain(#[from] crate::Error),
}
