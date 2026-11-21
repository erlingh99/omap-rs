//! Point symbols, and the point patterns line and area symbols embed: lists
//! of dots, circles, lines and areas.

use geo_types::{Coord, Point};

use std::io::Cursor;

use binrw::BinRead as _;

use super::raw::{
    BaseSymbol, Element as FileElement, ElementHeader, LineTail, PointTail, read_elements,
};
use super::{Reader, line_styles};
use crate::Code;
use crate::colors::SymbolColor;
use crate::objects::{AreaObject, LineObject, PointObject};
use crate::ocd::Result;
use crate::ocd::geometry::{read_polygon, read_rings};
use crate::ocd::units::length;
use crate::symbols::{AreaSymbol, Element, LineSymbol, PointSymbol, SymbolCommon};

impl Reader<'_> {
    /// A point symbol.
    pub(super) fn point_symbol(
        &mut self,
        common: SymbolCommon,
        base: &BaseSymbol,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<PointSymbol> {
        let tail = PointTail::read_le(cursor)?;
        let elements = read_elements(cursor, tail.data_size)?;
        let mut symbol = self.point_pattern(common.code, &elements);
        symbol.common = common;
        symbol.is_rotatable = base.flags & BaseSymbol::ROTATABLE != 0;
        Ok(symbol)
    }

    /// The point symbol `elements` draw, for the symbol numbered `code`.
    ///
    /// As Mapper does, the first dot or circle centered on the origin becomes
    /// the symbol's own circle; every other element is embedded.
    pub(super) fn point_pattern(&mut self, code: Code, elements: &[FileElement]) -> PointSymbol {
        let mut symbol = PointSymbol::new(Code::default(), "");
        let mut own_circle_used = false;
        for element in elements {
            let header = &element.header;
            let at_origin = element
                .coords
                .first()
                .is_none_or(|coord| coord.coord() == Coord::zero());
            match header.element_type {
                ElementHeader::DOT | ElementHeader::CIRCLE => {
                    let Some(circle) = self.circle(header) else {
                        continue;
                    };
                    if !own_circle_used && at_origin {
                        own_circle_used = true;
                        circle.apply_to(&mut symbol);
                    } else {
                        let mut embedded =
                            PointSymbol::new(Code::default(), "").with_rotatable(false);
                        circle.apply_to(&mut embedded);
                        let position = element
                            .coords
                            .first()
                            .map_or_else(Coord::zero, |c| c.coord());
                        symbol.elements.push(Element::Point {
                            symbol: Box::new(embedded),
                            object: Box::new(PointObject::new_element(Point(position))),
                        });
                    }
                }
                ElementHeader::LINE => {
                    if header.line_width <= 0 {
                        continue;
                    }
                    let (cap_style, join_style) = match header.flags {
                        ElementHeader::ROUND => line_styles(LineTail::ROUND_ROUND),
                        ElementHeader::FLAT_MITER => line_styles(LineTail::MITER_FLAT),
                        _ => line_styles(LineTail::BEVEL_FLAT),
                    }
                    .unwrap_or_default();
                    let line = LineSymbol::new(Code::default(), "")
                        .with_color(self.color(header.color))
                        .with_line_width(length(header.line_width))
                        .with_cap_style(cap_style)
                        .with_join_style(join_style);
                    for ring in read_rings(&element.coords) {
                        symbol.elements.push(Element::Line {
                            symbol: Box::new(line.clone()),
                            object: Box::new(LineObject::new_element(ring)),
                        });
                    }
                }
                ElementHeader::AREA => {
                    let area =
                        AreaSymbol::new(Code::default(), "").with_color(self.color(header.color));
                    if let Some(polygon) = read_polygon(&element.coords) {
                        symbol.elements.push(Element::Area {
                            symbol: Box::new(area),
                            object: Box::new(AreaObject::new_element(polygon)),
                        });
                    }
                }
                _ => self.symbol_issue(code, "an element of an unknown type was skipped"),
            }
        }
        symbol
    }

    /// The circle a dot or circle element draws, if it draws anything.
    fn circle(&mut self, header: &ElementHeader) -> Option<Circle> {
        if header.element_type == ElementHeader::DOT {
            return (header.diameter > 0).then(|| Circle {
                inner_color: self.color(header.color),
                inner_radius: f64::from(header.diameter) / 2.,
                outer_color: SymbolColor::NoColor,
                outer_width: 0.,
            });
        }
        // The diameter includes the line width once; Mapper reads the radius
        // as the distance to the middle of the line.
        let radius = (f64::from(header.diameter) - f64::from(header.line_width)) / 2.;
        (radius > 0. && header.line_width > 0).then(|| Circle {
            inner_color: SymbolColor::NoColor,
            inner_radius: radius,
            outer_color: self.color(header.color),
            outer_width: f64::from(header.line_width),
        })
    }
}

/// A point symbol's own circle, with lengths in file units.
struct Circle {
    inner_color: SymbolColor,
    inner_radius: f64,
    outer_color: SymbolColor,
    outer_width: f64,
}

impl Circle {
    fn apply_to(self, symbol: &mut PointSymbol) {
        symbol.inner_color = self.inner_color;
        symbol.inner_radius = length(self.inner_radius);
        symbol.outer_color = self.outer_color;
        symbol.outer_width = length(self.outer_width);
    }
}

/// Whether `symbol` draws nothing.
pub(super) fn is_empty(symbol: &PointSymbol) -> bool {
    symbol.elements.is_empty()
        && (symbol.inner_color == SymbolColor::NoColor || symbol.inner_radius.get() <= 0.)
        && (symbol.outer_color == SymbolColor::NoColor || symbol.outer_width.get() <= 0.)
}
