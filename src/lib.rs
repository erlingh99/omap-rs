//! A Rust library for reading and writing `OpenOrienteering` Mapper (`.omap`) files.
//!
//! All map coordinates are given in millimetres on paper, relative to the
//! reference point, with the positive y-axis pointing towards magnetic north.

/// The ordered, copy-on-write collection every set in a map is.
pub mod collection;
/// Color definitions: color set, spot colors, mixed colors, CMYK, RGB.
pub mod colors;
pub mod format;
/// Coordinate-reference-system and projection helpers.
pub mod geo_referencing;
/// Map objects: points, lines, areas, and text.
pub mod objects;
/// The top-level OMAP document type.
pub mod omap;
/// Map parts (layers) and their contained objects.
pub mod parts;
mod references;
/// Symbol definitions: point, line, area, text, and combined symbols.
pub mod symbols;
/// Background-template support (images, tracks, GDAL/OGR layers).
pub mod templates;
mod utils;
/// View settings: zoom, grid, template visibility.
pub mod view;
#[cfg(feature = "xml")]
pub mod xml;

use std::fmt::Debug;

pub use collection::{Collection, Entry, Removed};
pub use format::Format;
pub use geo_referencing::Transformable;
/// Re-export `geo_types` for convenience for downstream users
pub use geo_types;
pub use omap::Omap;
pub use utils::{Code, NonNegativeF64, PositiveF64, UnitF64};

pub type Result<T> = std::result::Result<T, Error>;

use thiserror::Error;

/// Errors that can occur when reading, writing, or manipulating OMAP data.
#[derive(Debug, Error)]
pub enum Error {
    /// A `&str` UTF-8 conversion error.
    #[error(transparent)]
    StrUtf8Error(#[from] std::str::Utf8Error),
    /// A position is past the end of a collection.
    #[error("position {position} is out of range for a collection of {len}")]
    PositionOutOfRange {
        /// The position asked for.
        position: usize,
        /// The number of items in the collection.
        len: usize,
    },
    /// Map parts could not be merged (indices out of range or identical).
    #[error("Could not merge map parts. Check that the indices are different and in range")]
    MapPartMergeError,
    /// Failed to parse an integer.
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    /// The georeferencing section could not be parsed.
    #[error("invalid georeferencing")]
    InvalidGeoreferencing,
    /// A Bézier-curve conversion error.
    #[error(transparent)]
    BezierConversionError(#[from] linestring2bezier::Error),
    /// An invalid color definition.
    #[error("Color definition error")]
    ColorError,
    /// A handle does not name a color of the kind the operation requires.
    #[error("expected a color handle of kind {expected:?}, found {found:?}")]
    ColorKindMismatch {
        /// The kinds the target handle type accepts.
        expected: &'static [colors::ColorKind],
        /// The kind the handle actually names.
        found: colors::ColorKind,
    },
    /// A symbol definition would create a cycle.
    #[error("cyclic symbol definition")]
    CyclicSymbolDefinition,
    /// Could not convert signed integer to unsigned integer
    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),
    /// An object-related error.
    #[error("Object error")]
    ObjectError,
    /// A Bézier path has disconnected segments or mismatched dash-point metadata.
    #[error("invalid Bézier path")]
    InvalidBezierPath,
    /// A flattened path has mismatched dash-point metadata.
    #[error("invalid flattened path")]
    InvalidFlattenedPath,
    /// A polygon contains a nonempty ring that is not closed.
    #[error("polygon rings must be closed")]
    OpenPolygonRing,
    /// The value is not in the unit interval `[0, 1]`.
    #[error("The value is not in the unit interval and cannot be converted to a UnitF64")]
    NotInUnitInterval,
    /// The value is negative.
    #[error("The value is not non-negative and cannot be converted to a NonNegativeF64")]
    NotNonNegativeF64,
    /// The value is negative or 0.
    #[error("The value is negative or 0 and cannot be converted to a PositiveF64")]
    NotPositiveF64,
    /// Infallible conversion (required by `From` blanket impl).
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    /// An Error when parsing a [Code] from a bad string
    #[error("Tried to parse a Code from a bad string")]
    BadCode,
    /// An error from the World Magnetic Model.
    #[cfg(feature = "geo_ref")]
    #[error(transparent)]
    WmmError(#[from] world_magnetic_model::Error),
    /// A proj4 projection error.
    #[cfg(feature = "geo_ref")]
    #[error(transparent)]
    ProjError(#[from] proj_core::Error),
    /// Could not parse the crs definition to a `CrsDef` object
    #[cfg(feature = "geo_ref")]
    #[error(transparent)]
    ProjParseError(#[from] proj_wkt::ParseError),
    /// A tolerance error when calculating the scale factor during geo referencing
    #[cfg(feature = "geo_ref")]
    #[error(
        "Encountered a tolerence error when calculating the scale factor during geo referencing"
    )]
    ProjScaleToleranceError,
    /// The local CRS type denotes the absence of a CRS, so it has no definition
    #[cfg(feature = "geo_ref")]
    #[error("The local CRS type has no CRS definition")]
    LocalCrsHasNoDefinition,
    /// Only affine transforms within the same projection are available without the `geo_ref` feature
    #[error("Failed to get a coordinate transform between the old and new GeoRef")]
    CannotGetTransformBetweenDifferentGeoRef,
    /// A handle does not name a symbol of the kind the operation requires.
    #[error("expected a symbol handle of kind {expected:?}, found {found:?}")]
    SymbolKindMismatch {
        /// The kinds the target handle type accepts.
        expected: &'static [symbols::SymbolKind],
        /// The kind the handle actually names.
        found: symbols::SymbolKind,
    },
    /// A handle does not resolve to a symbol of the required kind in this set.
    #[error("the handle does not name a symbol of the required kind in this set")]
    SymbolConversionError,
    /// Tried to do a transform to WGS84, but the transform could not be done as no there were no available transform between the map coordinates and WGS84
    #[cfg(feature = "geo_ref")]
    #[error("Tried a conversion to WGS84, but no transform to WGS84 is available")]
    NoWGS84TransformAvailable,
    #[error(transparent)]
    ValidationError(#[from] ValidationError),
}

/// The reason an [`Omap`] cannot be safely serialized without losing references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// A mixed color has a component whose spot color is not owned by the
    /// map's color set.
    #[error("color at index {color_index} mixes in a color not in the color set")]
    DanglingColorComponent {
        /// The index (priority) of the mixed color in its color set.
        color_index: usize,
    },
    /// A symbol references a color that is not owned by the map's color set.
    #[error("symbol at index {symbol_index} references a color not in the color set")]
    DanglingSymbolColor {
        /// The index of the symbol in its symbol set.
        symbol_index: usize,
    },
    /// A public part of a combined symbol names a symbol not owned by the
    /// symbol set.
    #[error("combined symbol at index {symbol_index} has a part not in the symbol set")]
    DanglingCombinedComponent {
        /// The index of the combined symbol in its symbol set.
        symbol_index: usize,
    },
    /// An object references a symbol that is not owned by the symbol set.
    #[error("object at index {object_index} references a symbol not in the symbol set")]
    DanglingObjectSymbol {
        /// The object's index in the map-wide object iterator.
        object_index: usize,
    },
}
