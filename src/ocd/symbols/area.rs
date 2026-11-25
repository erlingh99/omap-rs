//! Area symbols: a fill, hatching, a structure of point symbols, and a
//! border drawn by another symbol.
//!
//! An area with a border becomes a combined area symbol, as Mapper reads it:
//! the area as its private part, then the border line symbol as its public
//! part, which is resolved once every symbol is read.

use std::io::Cursor;

use binrw::BinRead as _;

use super::raw::{AreaTail, BaseSymbol, read_elements};
use super::{ReadSymbol, Reader};
use crate::NonNegativeF64;
use crate::colors::SymbolColor;
use crate::ocd::Result;
use crate::ocd::units::{length, radians};
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, ClippingOption, CombinedAreaSymbol, FillPattern,
    PublicOrPrivateSymbol, SymbolCommon,
};

impl Reader<'_> {
    pub(super) fn area_symbol(
        &mut self,
        common: SymbolCommon,
        base: &BaseSymbol,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ReadSymbol> {
        let tail = AreaTail::read_le(cursor)?;
        let elements = read_elements(cursor, tail.data_size)?;
        let code = common.code;

        let mut area = AreaSymbol::new(code, common.name.clone());
        area.common = common.clone();
        area.is_rotatable = base.flags & BaseSymbol::ROTATABLE != 0;
        area.color = if tail.fill_on != 0 {
            self.color(tail.fill_color)
        } else {
            SymbolColor::NoColor
        };

        if tail.hatch_mode != AreaTail::HATCH_NONE && tail.hatch_line_width > 0 {
            let color = self.color(tail.hatch_color);
            let hatch = |angle: i16| FillPattern::LinePattern {
                angle: radians(angle),
                line_spacing: length(tail.hatch_distance),
                line_offset: NonNegativeF64::zero(),
                line_color: color,
                line_width: length(tail.hatch_line_width),
                rotatable: true,
            };
            area.patterns.push(hatch(tail.hatch_angle_1));
            if tail.hatch_mode == AreaTail::HATCH_CROSS {
                area.patterns.push(hatch(tail.hatch_angle_2));
            }
        }

        if tail.structure_mode != AreaTail::STRUCTURE_NONE
            && tail.structure_width > 0
            && tail.structure_height > 0
            && !elements.is_empty()
        {
            self.structure(&mut area, &tail, &elements);
        }

        if tail.border_on == 0 {
            return Ok(ReadSymbol::new(area));
        }
        if tail.border_symbol == base.number {
            self.symbol_issue(code, "the border names the area itself and was skipped");
            return Ok(ReadSymbol::new(area));
        }
        let mut combined = CombinedAreaSymbol::new(code, common.name.clone());
        combined.common = common;
        combined.parts = vec![
            PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(area)),
            PublicOrPrivateSymbol::Public(None),
        ];
        let mut read = ReadSymbol::new(combined);
        read.border = Some(tail.border_symbol);
        Ok(read)
    }

    /// Add the structure's point patterns. Shifted rows become two patterns of
    /// twice the row spacing, the second offset by half a row and half a
    /// point, as Mapper reads them.
    fn structure(
        &mut self,
        area: &mut AreaSymbol,
        tail: &AreaTail,
        elements: &[super::raw::Element],
    ) {
        let code = area.common.code;
        if tail.structure_variation_x != 0 || tail.structure_variation_y != 0 {
            self.symbol_issue(code, "the structure's irregular variation is not supported");
        }
        let clip_options = match tail.structure_draw {
            1 => ClippingOption::NoClippingIfCompletelyInside,
            2 => ClippingOption::NoClippingIfCenterInside,
            3 => ClippingOption::NoClippingIfPartiallyInside,
            _ => ClippingOption::ClipElementsAtBoundary,
        };
        let point = self.point_pattern(code, elements);
        let shifted = tail.structure_mode == AreaTail::STRUCTURE_SHIFTED;
        let rows = if shifted { 2. } else { 1. };
        let line_spacing = length(f64::from(tail.structure_height) * rows);
        let pattern = |line_offset, offset_along_line| FillPattern::PointPattern {
            clip_options,
            angle: radians(tail.structure_angle),
            line_spacing,
            line_offset,
            offset_along_line,
            point_distance: length(tail.structure_width),
            point: point.clone(),
            rotatable: true,
        };
        area.patterns
            .push(pattern(NonNegativeF64::zero(), NonNegativeF64::zero()));
        if shifted {
            area.patterns.push(pattern(
                length(tail.structure_height),
                length(f64::from(tail.structure_width) / 2.),
            ));
        }
    }
}
