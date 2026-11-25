//! What reading or writing the OCAD format could not carry over exactly.

use crate::Code;

/// Something the OCAD format and the map model could not exchange exactly.
///
/// Reading and writing never fail on these: they skip or approximate the
/// feature and report it here, so the caller can decide whether the result is
/// good enough.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Warning {
    /// The file refers to a color number it does not define. The reference
    /// reads as no color.
    #[error("color number {0} is not defined; using no color")]
    UnknownColor(i32),
    /// The file refers to a symbol number it does not define. The reference
    /// reads as no symbol.
    #[error("symbol number {0} is not defined; using no symbol")]
    UnknownSymbol(i32),
    /// A symbol of a type the map model has no counterpart for was skipped.
    #[error("symbol {code}: type {symbol_type} is not supported; skipped")]
    UnsupportedSymbolType {
        /// The symbol's code.
        code: Code,
        /// The file's symbol type value.
        symbol_type: u8,
    },
    /// Part of a symbol's definition was dropped or approximated.
    #[error("symbol {code}: {issue}")]
    Symbol {
        /// The symbol's code.
        code: Code,
        /// What was lost.
        issue: &'static str,
    },
    /// An object was dropped or changed.
    #[error("object {index}: {issue}")]
    Object {
        /// The object's position among the objects of the file or map.
        index: usize,
        /// What was lost.
        issue: &'static str,
    },
    /// Something about the map as a whole was dropped or approximated.
    #[error("{0}")]
    Map(&'static str),
}

/// The warnings collected while reading or writing one map.
pub(in crate::ocd) type Warnings = Vec<Warning>;

/// Report `warning` unless it already was.
pub(in crate::ocd) fn push_once(warnings: &mut Warnings, warning: Warning) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}
