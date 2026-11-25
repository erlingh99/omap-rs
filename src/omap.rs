use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::num::NonZeroU32;
use std::path::Path;

use crate::references::{Live, Reference, References as _};

use geo_types::Coord;

use crate::{
    colors::ColorSet,
    format,
    geo_referencing::{GeoRef, MapTransform},
    objects::MapObject,
    parts::MapParts,
    symbols::SymbolSet,
    templates::Templates,
    view::View,
    {Result, ValidationError},
};

/// All objects are in map coordinates i.e given in mm of paper
/// relative the ref point with positive y towards the magnetic north
///
/// The Undo/Redo history and printer information is ignored
///
/// Cloning is cheap: colors, symbols, parts and objects are shared between the
/// copies, and changing one copy copies only the items the change touches, so
/// a copy can be handed to another thread while the original is edited.
/// Removing a color, symbol, part or object leaves its handle reserved, so it
/// can be restored under that handle and references to it resolve again.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Omap {
    /// Free-text notes embedded in the file.
    pub notes: String,
    /// Georeferencing information (scale, CRS, reference points).
    pub geo_referencing: GeoRef,
    /// The ordered set of colors used by symbols.
    pub colors: ColorSet,
    /// The set of map symbols.
    pub symbols: SymbolSet,
    /// The name of the symbol set, such as `ISOM 2017-2`.
    pub symbol_set_name: String,
    /// The map parts (layers) containing objects.
    pub parts: MapParts,
    /// Background templates attached to the map.
    pub templates: Templates,
    /// View settings (zoom, grid, visibility).
    pub view: View,
}

impl Omap {
    /// Create a new empty map
    pub fn new(scale_denominator: NonZeroU32) -> Self {
        Self {
            notes: Default::default(),
            geo_referencing: GeoRef::new(scale_denominator),
            colors: ColorSet::new(),
            symbols: SymbolSet::new(),
            symbol_set_name: "Custom".to_owned(),
            parts: MapParts::new_with_default_part(),
            templates: Default::default(),
            view: Default::default(),
        }
    }

    /// Read a map from a file.
    ///
    /// The format is chosen by the file's extension; see [`crate::format`].
    ///
    /// # Errors
    ///
    /// Returns an error if no format claims the file's extension, the file
    /// cannot be opened, or it is not a valid file of its format.
    pub fn from_path(path: impl AsRef<Path>) -> std::result::Result<Self, format::Error> {
        let path = path.as_ref();
        let format = format_for(path)?;
        let file = File::open(path)?;
        format.read(&mut BufReader::new(file))
    }

    /// Write the map to a file, in the format its extension names; see
    /// [`crate::format`].
    ///
    /// The replacement is atomic on platforms where [`std::fs::rename`] atomically
    /// replaces an existing destination. The temporary file is created beside
    /// `path`, so it is always on the same filesystem as the destination.
    ///
    /// # Errors
    ///
    /// Returns an error if no format claims the file's extension, the
    /// temporary file cannot be created, the map cannot be written in that
    /// format, or the temporary file cannot replace `path`.
    pub fn to_file(&self, path: impl AsRef<Path>) -> std::result::Result<(), format::Error> {
        let path = path.as_ref();
        let format = format_for(path)?;

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file_name = path.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OMAP output path must name a file",
            )
        })?;

        let mut temporary_name = file_name.to_os_string();
        temporary_name.push(format!(".omap-rs-{}.tmp", std::process::id()));
        let temporary_path = parent.join(temporary_name);
        let temporary_file = File::create(&temporary_path)?;

        let write_result = {
            let mut writer = BufWriter::new(temporary_file);
            format.write(self, &mut writer)
        };
        if let Err(error) = write_result {
            std::fs::remove_file(temporary_path)?;
            return Err(error);
        }

        if let Err(error) = std::fs::rename(&temporary_path, path) {
            std::fs::remove_file(temporary_path)?;
            return Err(error.into());
        }
        Ok(())
    }

    /// Iterate through all objects of every map part in a flat iterator.
    ///
    /// The whole-map counterpart of [`crate::parts::MapPart::objects`]; parts are
    /// visited in order, objects within a part in file order.
    pub fn iter_all_objects(&self) -> impl Iterator<Item = &MapObject> {
        self.parts.values().flat_map(|part| part.objects.values())
    }

    /// Iterate mutably through all objects of every map part in a flat iterator.
    ///
    /// The whole-map counterpart of [`crate::parts::MapPart::objects`]; parts are
    /// visited in order, objects within a part in file order. On a map
    /// that shares its objects with a copy, this copies every object.
    pub fn iter_all_objects_mut(&mut self) -> impl Iterator<Item = &mut MapObject> {
        self.parts
            .values_mut()
            .flat_map(|part| part.objects.values_mut())
    }

    /// Transform every object and non-georeferenced template in the map.
    ///
    /// Use this after changing the georeferencing
    /// to keep objects and non-georeferenced templates at the same real-world
    /// positions. Obtain the transform with
    /// [`MapTransform::transform_between`].
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        for object in self.iter_all_objects_mut() {
            object.transform(&transform);
        }
        self.templates.transform(transform);
    }

    /// Try to transform every object and non-georeferenced template in the map.
    ///
    /// Use this after changing the georeferencing to keep objects and
    /// non-georeferenced templates at the same real-world positions. The map is
    /// unchanged on failure.
    ///
    /// # Errors
    ///
    /// Returns any error produced while transforming an object or template.
    pub fn try_transform<E, F>(&mut self, transform: F) -> std::result::Result<(), E>
    where
        F: Fn(Coord) -> std::result::Result<Coord, E>,
    {
        let mut parts = self.parts.clone();
        for part in parts.values_mut() {
            for object in part.objects.values_mut() {
                object.try_transform(&transform)?;
            }
        }
        let mut templates = self.templates.clone();
        templates.try_transform(transform)?;

        self.parts = parts;
        self.templates = templates;
        Ok(())
    }

    /// Compute the transform between two [`MapTransform`]s and apply it
    /// to every object and non-georeferenced template. This is a convenience
    /// wrapper around [`MapTransform::transform_between`] + [`Omap::try_transform`].
    ///
    /// # Errors
    ///
    /// Returns an error if the transforms cannot be related or a coordinate
    /// cannot be transformed. The map is unchanged on failure.
    pub fn try_transform_between(&mut self, old: &MapTransform, new: &MapTransform) -> Result<()> {
        let transform = MapTransform::transform_between(old, new)?;
        self.try_transform(transform)
    }

    /// Drop every reference to a symbol or color that is no longer in its set.
    ///
    /// Follows what Mapper does when a color or symbol is deleted: a dangling
    /// color reference becomes [`crate::colors::SymbolColor::NoColor`], a
    /// dangling symbol reference (an object's, or a combined symbol's public
    /// part) becomes `None`, which writes as the format's `-1`, and a dangling
    /// mixed-color component is removed.
    ///
    /// Handles you already hold stay valid across this call — only the map's own
    /// references to removed values change. Afterwards [`Omap::validate`]
    /// passes. Only values that held a dangling reference are changed, so a
    /// copy of the map keeps sharing the rest.
    pub fn prune_dangling_references(&mut self) {
        let live = Live::of(&self.colors, &self.symbols);

        for color in self.colors.values_mut_where(|color| color.dangles(&live)) {
            color.prune(&live);
        }
        for symbol in self
            .symbols
            .values_mut_where(|symbol| symbol.dangles(&live))
        {
            symbol.prune(&live);
        }
        let dangles = |object: &MapObject| object.dangles(&live);
        for part in self
            .parts
            .values_mut_where(|part| part.objects.values().any(dangles))
        {
            for object in part.objects.values_mut_where(dangles) {
                object.prune(&live);
            }
        }
    }

    /// Check that every reference to a color or symbol resolves in this map.
    ///
    /// A reference is reported on the value that holds it: a color a public
    /// part of a combined symbol draws with is that part's, not the combined
    /// symbol's.
    ///
    /// # Errors
    ///
    /// Returns the first dangling reference with the map location of the value
    /// holding it. [`Omap::prune_dangling_references`] removes every one.
    pub fn validate(&self) -> std::result::Result<(), ValidationError> {
        let live = Live::of(&self.colors, &self.symbols);

        for (color_index, color) in self.colors.values().enumerate() {
            if color.dangles(&live) {
                return Err(ValidationError::DanglingColorComponent { color_index });
            }
        }
        for (symbol_index, symbol) in self.symbols.values().enumerate() {
            match symbol.first_dangling(&live) {
                Some(Reference::Color(_)) => {
                    return Err(ValidationError::DanglingSymbolColor { symbol_index });
                }
                Some(Reference::Symbol(_)) => {
                    return Err(ValidationError::DanglingCombinedComponent { symbol_index });
                }
                None => (),
            }
        }
        for (object_index, object) in self.iter_all_objects().enumerate() {
            if object.dangles(&live) {
                return Err(ValidationError::DanglingObjectSymbol { object_index });
            }
        }
        Ok(())
    }
}

/// The format that claims `path`'s extension.
fn format_for(path: &Path) -> std::result::Result<&'static dyn format::Format, format::Error> {
    format::for_path(path).ok_or_else(|| {
        format::Error::UnsupportedExtension(
            path.extension()
                .map(|extension| extension.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    })
}
