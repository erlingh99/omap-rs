use super::{Cmyk, Color, ColorId, MixedColor, MixedColorId, Rgb, SpotColor, SpotColorId};

use crate::{Collection, Entry, Error, Result};

/// The map's colors, in priority order.
///
/// A color's position in the set is its priority, which is what the `.omap`
/// format stores. A [`ColorId`] keeps naming the same color across
/// [`Collection::swap`], [`Collection::insert`] and [`Collection::remove`];
/// see [`Collection`] for everything a set does.
///
/// A [`ColorId`] left over from a removed color contributes no color when the
/// map is written.
pub type ColorSet = Collection<ColorId, Color>;

impl ColorSet {
    /// Get a spot color by its handle. Cannot return a mixed color.
    pub fn spot_color(&self, id: SpotColorId) -> Option<&SpotColor> {
        match self.get(id)?.value() {
            Color::SpotColor(color) => Some(color),
            Color::MixedColor(_) => None,
        }
    }

    /// Mutably get a spot color by its handle.
    pub fn spot_color_mut(&mut self, id: SpotColorId) -> Option<&mut SpotColor> {
        match self.get_mut(id)? {
            Color::SpotColor(color) => Some(color),
            Color::MixedColor(_) => None,
        }
    }

    /// Get a mixed color by its handle. Cannot return a spot color.
    pub fn mixed_color(&self, id: MixedColorId) -> Option<&MixedColor> {
        match self.get(id)?.value() {
            Color::MixedColor(color) => Some(color),
            Color::SpotColor(_) => None,
        }
    }

    /// Mutably get a mixed color by its handle.
    pub fn mixed_color_mut(&mut self, id: MixedColorId) -> Option<&mut MixedColor> {
        match self.get_mut(id)? {
            Color::MixedColor(color) => Some(color),
            Color::SpotColor(_) => None,
        }
    }

    /// Find a color by exact name match. The first match is returned.
    pub fn find_by_name(&self, name: &str) -> Option<Entry<'_, ColorId, Color>> {
        self.iter().find(|color| color.name() == name)
    }

    /// Get the effective CMYK value of a color in this set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `id` is not in this set or the color
    /// definition cannot produce a CMYK value.
    pub fn cmyk(&self, id: ColorId) -> Result<Cmyk> {
        self.get(id).ok_or(Error::ColorError)?.cmyk(self)
    }

    /// Get the effective RGB value of a color in this set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `id` is not in this set or the color
    /// definition cannot produce an RGB value.
    pub fn rgb(&self, id: ColorId) -> Result<Rgb> {
        self.get(id).ok_or(Error::ColorError)?.rgb(self)
    }
}

/// Narrowing a looked-up color to a typed handle.
///
/// ```
/// # use omap::Omap;
/// # fn example(map: &Omap) {
/// let purple = map.colors.find_by_name("Purple").and_then(|color| color.as_spot());
/// # }
/// ```
impl Entry<'_, ColorId, Color> {
    /// Narrow to a spot color handle, or `None` if this names a mixed color.
    pub fn as_spot(self) -> Option<SpotColorId> {
        SpotColorId::try_from(self).ok()
    }

    /// Narrow to a mixed color handle, or `None` if this names a spot color.
    pub fn as_mixed(self) -> Option<MixedColorId> {
        MixedColorId::try_from(self).ok()
    }
}
