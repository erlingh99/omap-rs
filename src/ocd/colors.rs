//! Colors: type 9 parameter strings for colors and type 10 for spot color
//! separations.
//!
//! The file names a color by its number, not by its position. The order of
//! the color records is the drawing order, top first, which becomes the
//! colors' priority. Separations become spot colors, placed after every
//! color and sorted by number, as Mapper does; colors become mixed colors of
//! those spot colors.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use crate::colors::{
    Cmyk, CmykMode, Color, ColorComponent, ColorId, ColorSet, MixedColor, MixedColorId, RgbMode,
    SpotColor, SpotColorId, SymbolColor,
};
use crate::ocd::strings::{self, ParameterString, kind};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::ocd::{Error, Result};
use crate::symbols::SymbolSet;
use crate::{NonNegativeF64, UnitF64};

/// The name Mapper and OCAD give the registration color.
const REGISTRATION_BLACK: &str = "Registration black";

/// How far CMYK may differ from the one its spot colors make and still be
/// taken as made by them: half a percent.
const CMYK_TOLERANCE: f64 = 0.005;

/// The file's color numbers, and the colors they name.
#[derive(Debug, Default)]
pub(in crate::ocd) struct ColorNumbers(HashMap<i32, SymbolColor>);

impl ColorNumbers {
    /// The color `number` names. A number the file does not define is no
    /// color, and is reported once.
    pub(in crate::ocd) fn get(&self, number: i32, warnings: &mut Warnings) -> SymbolColor {
        if let Some(color) = self.0.get(&number) {
            return *color;
        }
        push_once(warnings, Warning::UnknownColor(number));
        SymbolColor::NoColor
    }
}

/// A separation as the file describes it.
struct Separation {
    number: i32,
    color: SpotColor,
}

/// Read the color set from the file's parameter strings.
pub(in crate::ocd) fn read_colors(
    strings: &[ParameterString],
    warnings: &mut Warnings,
) -> (ColorSet, ColorNumbers) {
    let mut separations: Vec<Separation> = strings
        .iter()
        .filter(|string| string.kind == kind::SPOT_COLOR)
        .filter_map(read_separation)
        .collect();
    separations.sort_by_key(|separation| separation.number);

    let mut colors = ColorSet::new();
    let mut numbers = ColorNumbers::default();
    let mut pending_components = Vec::new();
    for string in strings.iter().filter(|string| string.kind == kind::COLOR) {
        let Some(read) = read_color(string, &separations, warnings) else {
            continue;
        };
        if read.is_registration_black(separations.len()) {
            numbers
                .0
                .insert(read.number, SymbolColor::RegistrationBlack);
            continue;
        }
        let id = colors.push(Color::MixedColor(read.color));
        numbers.0.insert(read.number, SymbolColor::Color(id));
        pending_components.push((id, read.overprint, read.components));
    }

    let spot_ids: Vec<SpotColorId> = separations
        .into_iter()
        .map(|separation| SpotColorId(colors.push(Color::SpotColor(separation.color)).0))
        .collect();

    for (id, overprint, shares) in pending_components {
        let id = MixedColorId(id.0);
        if let Some(color) = colors.mixed_color_mut(id) {
            color.knockout = !overprint;
        }
        if shares.is_empty() {
            continue;
        }
        let components = shares
            .into_iter()
            .map(|(separation, factor)| ColorComponent {
                factor,
                color: spot_ids[separation],
            })
            .collect();
        if let Some(color) = colors.mixed_color_mut(id) {
            color.components = components;
        }
        let made_by_spot_colors = colors.mixed_color(id).is_some_and(|color| {
            let stated = color.cmyk(&colors);
            let from_spots = color.cmyk_from_spotcolors(&colors);
            matches!((stated, from_spots), (Ok(a), Ok(b)) if cmyk_close(a, b))
        });
        if made_by_spot_colors && let Some(color) = colors.mixed_color_mut(id) {
            color.cmyk_mode = CmykMode::FromSpotColors;
            color.rgb_mode = RgbMode::FromSpotColors;
        }
    }

    (colors, numbers)
}

fn cmyk_close(a: Cmyk, b: Cmyk) -> bool {
    [(a.c, b.c), (a.m, b.m), (a.y, b.y), (a.k, b.k)]
        .into_iter()
        .all(|(a, b)| (a.get() - b.get()).abs() <= CMYK_TOLERANCE)
}

/// A percentage field as a fraction, if it is a percentage.
fn percent(value: &str) -> Option<UnitF64> {
    strings::number(value)
        .filter(|value| (0.0..=100.0).contains(value))
        .map(|value| UnitF64::clamped_from(value / 100.))
}

/// Set the CMYK channel `code` names, if it names one.
fn set_channel(cmyk: &mut Cmyk, code: char, value: &str) {
    let Some(value) = percent(value) else {
        return;
    };
    match code {
        'c' => cmyk.c = value,
        'm' => cmyk.m = value,
        'y' => cmyk.y = value,
        'k' => cmyk.k = value,
        _ => (),
    }
}

fn read_separation(string: &ParameterString) -> Option<Separation> {
    let name = string.first();
    let mut number = None;
    let mut cmyk = Cmyk::default();
    // Mapper's defaults for a separation that states no screen.
    let mut screen_angle_deg = 45.;
    let mut screen_frequency = 150.;
    for (code, value) in string.fields() {
        match code {
            'n' => number = value.trim().parse().ok(),
            'c' | 'm' | 'y' | 'k' => set_channel(&mut cmyk, code, value),
            'f' => {
                if let Some(tenths) = number_at_least_zero(value) {
                    screen_frequency = tenths / 10.;
                }
            }
            'a' => {
                if let Some(angle) = number_at_least_zero(value) {
                    screen_angle_deg = angle;
                }
            }
            _ => (),
        }
    }
    let mut color = SpotColor::new(name, name, cmyk);
    color.screen_angle_deg = screen_angle_deg;
    color.screen_frequency = NonNegativeF64::clamped_from(screen_frequency);
    Some(Separation {
        number: number?,
        color,
    })
}

fn number_at_least_zero(value: &str) -> Option<f64> {
    strings::number(value).filter(|value| *value >= 0.)
}

/// A color as the file describes it, before its spot colors exist.
struct ReadColor {
    number: i32,
    color: MixedColor,
    overprint: bool,
    /// Shares of separations, by their position in the sorted separations.
    components: Vec<(usize, UnitF64)>,
}

impl ReadColor {
    /// Whether this is the registration color: so named with full black, or
    /// a full share of every separation with full CMYK, as Mapper decides.
    fn is_registration_black(&self, separation_count: usize) -> bool {
        let CmykMode::Cmyk(cmyk) = self.color.cmyk_mode else {
            return false;
        };
        let full = |channel: UnitF64| channel.get() >= 1.;
        // Mapper writes it with full CMYK, so full black is enough.
        (full(cmyk.k) && self.color.color_name == REGISTRATION_BLACK)
            || (!self.components.is_empty()
                && self.components.len() == separation_count
                && full(cmyk.c)
                && full(cmyk.m)
                && full(cmyk.y)
                && full(cmyk.k)
                && self.components.iter().all(|(_, factor)| full(*factor)))
    }
}

fn read_color(
    string: &ParameterString,
    separations: &[Separation],
    warnings: &mut Warnings,
) -> Option<ReadColor> {
    let mut number = None;
    let mut cmyk = Cmyk::default();
    let mut overprint = false;
    let mut components = Vec::new();
    let mut separation_name = None;
    for (code, value) in string.fields() {
        match code {
            'n' => number = value.trim().parse().ok(),
            'c' | 'm' | 'y' | 'k' => set_channel(&mut cmyk, code, value),
            'o' => overprint = value.trim().parse::<i32>().is_ok_and(|o| o != 0),
            't' => {
                if percent(value).is_some_and(|opacity| opacity.get() < 1.) {
                    push_once(
                        warnings,
                        Warning::Map("color transparency is not supported"),
                    );
                }
            }
            's' => separation_name = Some(value),
            'p' => {
                // A share belongs to the separation named just before it.
                let Some(name) = separation_name.take() else {
                    continue;
                };
                let Some(position) = separations
                    .iter()
                    .position(|separation| separation.color.color_name == name)
                else {
                    continue;
                };
                let share = strings::number(value).unwrap_or(0.).clamp(0., 100.) / 100.;
                components.push((position, UnitF64::clamped_from(share)));
            }
            _ => (),
        }
    }
    let mut color = MixedColor::new(string.first(), Vec::new());
    color.cmyk_mode = CmykMode::Cmyk(cmyk);
    color.rgb_mode = RgbMode::FromCmyk;
    Some(ReadColor {
        number: number?,
        color,
        overprint,
        components,
    })
}

/// The number registration black is written under, whether or not it is
/// used; the colors of the set are numbered from 1.
const REGISTRATION_NUMBER: u16 = 0;

/// How a map's colors are written: which are colors, which only
/// separations, and the numbers symbols name them by.
pub(in crate::ocd) struct ColorWriter {
    numbers: HashMap<ColorId, u16>,
    registration_used: Cell<bool>,
}

impl ColorWriter {
    /// Number the colors of `colors`.
    ///
    /// Every color is written as a color, except the spot colors after the
    /// last mixed color that no symbol uses: as Mapper writes them, and as
    /// reading leaves the separations of a file, they are only separations.
    pub(in crate::ocd) fn new(colors: &ColorSet, symbols: &SymbolSet) -> Result<Self> {
        let used: HashSet<ColorId> = symbols
            .values()
            .flat_map(|symbol| symbol.colors(symbols))
            .collect();
        let first_trailing_spot = colors
            .values()
            .rposition(|color| matches!(color, Color::MixedColor(_)))
            .map_or(0, |last_mixed| last_mixed + 1);
        let mut numbers = HashMap::new();
        let mut next = REGISTRATION_NUMBER + 1;
        for (position, entry) in colors.iter().enumerate() {
            let only_separation = position >= first_trailing_spot && !used.contains(&entry.id());
            if only_separation {
                continue;
            }
            numbers.insert(entry.id(), next);
            next = next.checked_add(1).ok_or(Error::TooMany("colors"))?;
        }
        Ok(Self {
            numbers,
            registration_used: Cell::new(false),
        })
    }

    /// The number `color` is written under, or `None` for no color.
    pub(in crate::ocd) fn number(&self, color: SymbolColor) -> Option<u16> {
        match color {
            SymbolColor::Color(id) => self.numbers.get(&id).copied(),
            SymbolColor::RegistrationBlack => {
                self.registration_used.set(true);
                Some(REGISTRATION_NUMBER)
            }
            SymbolColor::NoColor => None,
        }
    }

    /// The color and separation records, once the symbols have asked for
    /// every number they need.
    pub(in crate::ocd) fn strings(
        &self,
        colors: &ColorSet,
        warnings: &mut Warnings,
    ) -> Vec<ParameterString> {
        let separations: Vec<&SpotColor> = colors
            .values()
            .filter_map(|color| match color {
                Color::SpotColor(spot) => Some(spot),
                Color::MixedColor(_) => None,
            })
            .collect();
        let mut strings = Vec::new();
        if self.registration_used.get() {
            let full = Cmyk {
                c: UnitF64::one(),
                m: UnitF64::one(),
                y: UnitF64::one(),
                k: UnitF64::one(),
            };
            let mut string = color_string(REGISTRATION_BLACK, REGISTRATION_NUMBER, full, false);
            for separation in &separations {
                string.push('s', &separation.spotcolor_name);
                string.push('p', 100);
            }
            strings.push(string);
        }
        for entry in colors {
            let Some(&number) = self.numbers.get(&entry.id()) else {
                continue;
            };
            let cmyk = entry.cmyk(colors).unwrap_or_else(|_unresolvable| {
                push_once(
                    warnings,
                    Warning::Map("a color's CMYK could not be worked out; written as white"),
                );
                Cmyk::default()
            });
            let mut string = color_string(entry.name(), number, cmyk, entry.is_knockout());
            match entry.value() {
                Color::SpotColor(spot) => {
                    string.push('s', &spot.spotcolor_name);
                    string.push('p', 100);
                }
                Color::MixedColor(mixed) => {
                    for component in &mixed.components {
                        if let Some(spot) = colors.spot_color(component.color) {
                            string.push('s', &spot.spotcolor_name);
                            string.push('p', as_percent(component.factor.get()));
                        }
                    }
                }
            }
            strings.push(string);
        }
        for (number, separation) in separations.into_iter().enumerate() {
            let cmyk = separation.cmyk().unwrap_or_default();
            let mut string = ParameterString::new(kind::SPOT_COLOR, &separation.spotcolor_name);
            string.push('n', number);
            string.push('v', 1);
            push_cmyk(&mut string, cmyk);
            string.push('f', tenths(separation.screen_frequency.get() * 10.));
            string.push('a', tenths(separation.screen_angle_deg));
            strings.push(string);
        }
        strings
    }
}

fn color_string(name: &str, number: u16, cmyk: Cmyk, knockout: bool) -> ParameterString {
    let mut string = ParameterString::new(kind::COLOR, name);
    string.push('n', number);
    push_cmyk(&mut string, cmyk);
    string.push('o', i32::from(!knockout));
    string.push('t', 100);
    string
}

fn push_cmyk(string: &mut ParameterString, cmyk: Cmyk) {
    string.push('c', as_percent(cmyk.c.get()));
    string.push('m', as_percent(cmyk.m.get()));
    string.push('y', as_percent(cmyk.y.get()));
    string.push('k', as_percent(cmyk.k.get()));
}

/// A fraction as a percentage, to a tenth of a percent.
fn as_percent(fraction: f64) -> String {
    tenths(fraction * 100.)
}

/// A number to a tenth, without a trailing `.0`.
fn tenths(value: f64) -> String {
    let tenths = (value * 10.).round() / 10.;
    if tenths.fract() == 0. {
        format!("{tenths:.0}")
    } else {
        format!("{tenths:.1}")
    }
}

#[cfg(test)]
mod tests {
    use crate::colors::{Color, ColorSet, SymbolColor};
    use crate::ocd::Result;

    #[test]
    fn a_spot_color_a_symbol_cannot_use_is_only_a_separation() -> Result<()> {
        use crate::colors::{Cmyk, MixedColor, SpotColor};
        let mut colors = ColorSet::new();
        let middle = colors.push(Color::SpotColor(SpotColor::new(
            "Middle",
            "Middle",
            Cmyk::default(),
        )));
        let mixed = colors.push(Color::MixedColor(MixedColor::new("Mixed", Vec::new())));
        let trailing = colors.push(Color::SpotColor(SpotColor::new(
            "Trailing",
            "Trailing",
            Cmyk::default(),
        )));
        let writer = super::ColorWriter::new(&colors, &crate::symbols::SymbolSet::new())?;
        assert_eq!(writer.number(SymbolColor::Color(middle)), Some(1));
        assert_eq!(writer.number(SymbolColor::Color(mixed)), Some(2));
        assert_eq!(writer.number(SymbolColor::Color(trailing)), None);
        assert_eq!(writer.number(SymbolColor::NoColor), None);
        Ok(())
    }
}
