//! Reading and writing the OpenOrienteering Mapper XML format (`.omap`,
//! `.xmap`).
//!
//! The domain types know nothing of the format: everything about how a map is
//! laid out as XML, including how handles become the format's integer
//! indices, lives here.

pub(crate) mod colors;
pub(crate) mod format_info;
pub(crate) mod georef;
pub(crate) mod objects;
pub(crate) mod parts;
pub(crate) mod symbols;
pub(crate) mod templates;
mod text;
pub(crate) mod view;

pub(crate) use text::{read_text, write_notes};
