//! File formats a map is read from and written to.
//!
//! A [`Format`] turns a byte stream into an [`Omap`] and back. [`for_path`]
//! picks one by file extension, and [`all`] lists every format compiled into
//! this build, for an application's open and save dialogs.

use std::io::{Read, Write};
use std::path::Path;

use crate::Omap;

/// A file format a map can be read from and written to.
pub trait Format: Send + Sync {
    /// The file extensions this format reads and writes, lower case and
    /// without the dot.
    fn extensions(&self) -> &'static [&'static str];

    /// Read a map.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid file of this format, or
    /// cannot be read.
    fn read(&self, reader: &mut dyn Read) -> Result<Omap, Error>;

    /// Write `map`.
    ///
    /// What the format cannot hold exactly is approximated or left out, as
    /// that format's module documents; a module may also offer a write that
    /// reports what was approximated, such as `ocd::write`.
    ///
    /// # Errors
    ///
    /// Returns an error if the map cannot be written in this format at all,
    /// or the output cannot be written.
    fn write(&self, map: &Omap, writer: &mut dyn Write) -> Result<(), Error>;
}

static FORMATS: &[&dyn Format] = &[
    #[cfg(feature = "xml")]
    &crate::xml::Xml,
    #[cfg(feature = "ocd")]
    &crate::ocd::Ocd,
];

/// Every format compiled into this build.
pub fn all() -> &'static [&'static dyn Format] {
    FORMATS
}

/// The format that claims `path`'s extension, compared case-insensitively.
pub fn for_path(path: impl AsRef<Path>) -> Option<&'static dyn Format> {
    let extension = path.as_ref().extension()?.to_str()?.to_ascii_lowercase();
    all()
        .iter()
        .copied()
        .find(|format| format.extensions().contains(&extension.as_str()))
}

/// Why reading or writing a map file failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The `OpenOrienteering` Mapper XML format.
    #[cfg(feature = "xml")]
    #[error(transparent)]
    Xml(crate::xml::Error),
    /// The OCAD format.
    #[cfg(feature = "ocd")]
    #[error(transparent)]
    Ocd(crate::ocd::Error),
    /// Reading or writing the file itself.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// No format compiled into this build claims the file's extension.
    #[error("no format reads or writes files with the extension {0:?}")]
    UnsupportedExtension(String),
}
