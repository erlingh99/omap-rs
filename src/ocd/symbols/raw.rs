//! Symbol structures as the file lays them out.
//!
//! Every symbol starts with a [`BaseSymbol`]. The structure after it depends
//! on the symbol type; point symbol elements follow that.

use std::io::{Cursor, Read, Seek};

use binrw::{BinRead as _, BinResult, BinWrite as _, binrw};

use crate::ocd::units::FileCoord;

/// The symbol type values.
pub(in crate::ocd) mod symbol_type {
    pub(in crate::ocd) const POINT: u8 = 1;
    pub(in crate::ocd) const LINE: u8 = 2;
    pub(in crate::ocd) const AREA: u8 = 3;
    pub(in crate::ocd) const TEXT: u8 = 4;
    pub(in crate::ocd) const LINE_TEXT: u8 = 6;
    pub(in crate::ocd) const RECTANGLE: u8 = 7;
}

/// The part every symbol starts with.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub(in crate::ocd) struct BaseSymbol {
    /// The symbol's size in bytes, including what follows the base.
    pub(in crate::ocd) size: i32,
    /// 1000 times the symbol number: 203.145 is 203145.
    pub(in crate::ocd) number: i32,
    pub(in crate::ocd) symbol_type: u8,
    pub(in crate::ocd) flags: u8,
    pub(in crate::ocd) selected: u8,
    pub(in crate::ocd) status: u8,
    pub(in crate::ocd) tool: u8,
    pub(in crate::ocd) course_setting_mode: u8,
    pub(in crate::ocd) course_setting_type: u8,
    pub(in crate::ocd) control_description_flags: u8,
    pub(in crate::ocd) extent: i32,
    pub(in crate::ocd) file_position: u32,
    pub(in crate::ocd) unused: [u8; 2],
    pub(in crate::ocd) color_count: i16,
    pub(in crate::ocd) colors: [u16; 14],
    /// The symbol's name, zero-terminated UTF-16.
    pub(in crate::ocd) description: [u16; 64],
    pub(in crate::ocd) icon: [u8; 484],
    pub(in crate::ocd) groups: [u16; 64],
}

impl BaseSymbol {
    /// The base's size in the file.
    pub(in crate::ocd) const SIZE: usize = 796;

    /// Flag: the symbol may be rotated.
    pub(in crate::ocd) const ROTATABLE: u8 = 0x01;
    /// Status: the symbol is protected.
    pub(in crate::ocd) const PROTECTED: u8 = 0x01;
    /// Status: the symbol is hidden.
    pub(in crate::ocd) const HIDDEN: u8 = 0x02;

    /// A base for a symbol of `symbol_type` numbered `number`, all else
    /// zero.
    pub(in crate::ocd) fn new(number: i32, symbol_type: u8) -> Self {
        Self {
            size: 0,
            number,
            symbol_type,
            flags: 0,
            selected: 0,
            status: 0,
            tool: 0,
            course_setting_mode: 0,
            course_setting_type: 0,
            control_description_flags: 0,
            extent: 0,
            file_position: 0,
            unused: [0; 2],
            color_count: 0,
            colors: [0; 14],
            description: [0; 64],
            icon: [0; 484],
            groups: [0; 64],
        }
    }

    /// The symbol's name.
    pub(in crate::ocd) fn name(&self) -> String {
        let end = self
            .description
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(self.description.len());
        String::from_utf16_lossy(&self.description[..end])
    }
}

/// What follows the base of a point symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct PointTail {
    /// The size of the elements, in 8-byte units.
    pub(in crate::ocd) data_size: u16,
    pub(in crate::ocd) reserved: u16,
}

/// What follows the base of a line symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct LineTail {
    pub(in crate::ocd) line_color: u16,
    pub(in crate::ocd) line_width: u16,
    pub(in crate::ocd) line_style: u16,
    pub(in crate::ocd) distance_from_start: i16,
    pub(in crate::ocd) distance_to_end: i16,
    pub(in crate::ocd) main_length: i16,
    pub(in crate::ocd) end_length: i16,
    pub(in crate::ocd) main_gap: i16,
    pub(in crate::ocd) secondary_gap: i16,
    pub(in crate::ocd) end_gap: i16,
    pub(in crate::ocd) minimum_symbols: i16,
    pub(in crate::ocd) primary_symbol_count: i16,
    pub(in crate::ocd) primary_symbol_distance: i16,
    pub(in crate::ocd) double_mode: u16,
    pub(in crate::ocd) double_flags: u16,
    pub(in crate::ocd) double_fill_color: u16,
    pub(in crate::ocd) double_left_color: u16,
    pub(in crate::ocd) double_right_color: u16,
    pub(in crate::ocd) double_width: i16,
    pub(in crate::ocd) double_left_width: i16,
    pub(in crate::ocd) double_right_width: i16,
    pub(in crate::ocd) double_length: i16,
    pub(in crate::ocd) double_gap: i16,
    pub(in crate::ocd) double_background_color: u16,
    pub(in crate::ocd) reserved_1: [u16; 2],
    /// 0 off, 1 towards the end, 2 towards both ends, 3 towards the first
    /// symbol.
    pub(in crate::ocd) decrease_mode: u16,
    pub(in crate::ocd) decrease_symbol_size: i16,
    pub(in crate::ocd) decrease_distance: u8,
    pub(in crate::ocd) decrease_width: u8,
    pub(in crate::ocd) framing_color: u16,
    pub(in crate::ocd) framing_width: i16,
    pub(in crate::ocd) framing_style: u16,
    pub(in crate::ocd) primary_data_size: u16,
    pub(in crate::ocd) secondary_data_size: u16,
    pub(in crate::ocd) corner_data_size: u16,
    pub(in crate::ocd) start_data_size: u16,
    pub(in crate::ocd) end_data_size: u16,
    pub(in crate::ocd) active_symbols: u8,
    pub(in crate::ocd) reserved_3: u8,
}

impl LineTail {
    /// Line style: bevel joins and flat caps.
    pub(in crate::ocd) const BEVEL_FLAT: u16 = 0;
    /// Line style: round joins and round caps.
    pub(in crate::ocd) const ROUND_ROUND: u16 = 1;
    /// Line style: bevel joins and pointed caps.
    pub(in crate::ocd) const BEVEL_POINTED: u16 = 2;
    /// Line style: round joins and pointed caps.
    pub(in crate::ocd) const ROUND_POINTED: u16 = 3;
    /// Line style: miter joins and flat caps.
    pub(in crate::ocd) const MITER_FLAT: u16 = 4;
    /// Line style: miter joins and pointed caps.
    pub(in crate::ocd) const MITER_POINTED: u16 = 6;

    /// Double line mode: off.
    pub(in crate::ocd) const DOUBLE_OFF: u16 = 0;
    /// Double line mode: continuous.
    pub(in crate::ocd) const DOUBLE_CONTINUOUS: u16 = 1;
    /// Double line mode: the left border is dashed.
    pub(in crate::ocd) const DOUBLE_LEFT_DASHED: u16 = 2;
    /// Double line mode: both borders are dashed.
    pub(in crate::ocd) const DOUBLE_BORDERS_DASHED: u16 = 3;
    /// Double line mode: the borders and the filling are dashed.
    pub(in crate::ocd) const DOUBLE_ALL_DASHED: u16 = 4;

    /// Double line flag: the filling has a color.
    pub(in crate::ocd) const DOUBLE_FILL_ON: u16 = 0x01;
    /// Double line flag: the background color is on.
    pub(in crate::ocd) const DOUBLE_BACKGROUND_ON: u16 = 0x02;
}

/// What follows the base of an area symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct AreaTail {
    /// The number of the line symbol that draws the border.
    pub(in crate::ocd) border_symbol: i32,
    pub(in crate::ocd) fill_color: u16,
    pub(in crate::ocd) hatch_mode: u16,
    pub(in crate::ocd) hatch_color: u16,
    pub(in crate::ocd) hatch_line_width: u16,
    pub(in crate::ocd) hatch_distance: u16,
    pub(in crate::ocd) hatch_angle_1: i16,
    pub(in crate::ocd) hatch_angle_2: i16,
    pub(in crate::ocd) fill_on: u8,
    pub(in crate::ocd) border_on: u8,
    pub(in crate::ocd) structure_mode: u8,
    pub(in crate::ocd) structure_draw: u8,
    pub(in crate::ocd) structure_width: u16,
    pub(in crate::ocd) structure_height: u16,
    pub(in crate::ocd) structure_angle: i16,
    pub(in crate::ocd) structure_variation_x: u8,
    pub(in crate::ocd) structure_variation_y: u8,
    pub(in crate::ocd) structure_minimum_distance: u16,
    pub(in crate::ocd) reserved: u16,
    /// The size of the structure's elements, in 8-byte units.
    pub(in crate::ocd) data_size: u16,
}

impl AreaTail {
    /// Hatch mode: no hatching.
    pub(in crate::ocd) const HATCH_NONE: u16 = 0;
    /// Hatch mode: two sets of lines.
    pub(in crate::ocd) const HATCH_CROSS: u16 = 2;
    /// Structure mode: no structure.
    pub(in crate::ocd) const STRUCTURE_NONE: u8 = 0;
    /// Structure mode: aligned rows.
    pub(in crate::ocd) const STRUCTURE_ALIGNED: u8 = 1;
    /// Structure mode: every other row shifted by half the width.
    pub(in crate::ocd) const STRUCTURE_SHIFTED: u8 = 2;
}

/// A Pascal string of up to 31 bytes, UTF-8 in this version.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct PascalString31 {
    pub(in crate::ocd) length: u8,
    pub(in crate::ocd) data: [u8; 31],
}

impl PascalString31 {
    pub(in crate::ocd) fn text(&self) -> String {
        let length = usize::from(self.length).min(self.data.len());
        String::from_utf8_lossy(&self.data[..length]).into_owned()
    }

    /// `text`, cut to the longest prefix of whole characters that fits.
    pub(in crate::ocd) fn new(text: &str) -> Self {
        let mut length = text.len().min(31);
        while !text.is_char_boundary(length) {
            length -= 1;
        }
        let mut data = [0; 31];
        data[..length].copy_from_slice(&text.as_bytes()[..length]);
        Self {
            // At most 31.
            length: length as u8,
            data,
        }
    }
}

/// Font, color, size, weight, spacing and alignment of text.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct BasicText {
    pub(in crate::ocd) font_name: PascalString31,
    pub(in crate::ocd) color: u16,
    /// Tenths of a point.
    pub(in crate::ocd) font_size: u16,
    /// 400 normal, 700 bold.
    pub(in crate::ocd) weight: u16,
    pub(in crate::ocd) italic: u8,
    pub(in crate::ocd) charset: u8,
    /// Percent.
    pub(in crate::ocd) character_spacing: i16,
    /// Percent.
    pub(in crate::ocd) word_spacing: u16,
    pub(in crate::ocd) alignment: u16,
}

impl BasicText {
    pub(in crate::ocd) const HORIZONTAL_MASK: u16 = 0x03;
    pub(in crate::ocd) const LEFT: u16 = 0x00;
    pub(in crate::ocd) const CENTER: u16 = 0x01;
    pub(in crate::ocd) const RIGHT: u16 = 0x02;
    pub(in crate::ocd) const VERTICAL_MASK: u16 = 0x0c;
    pub(in crate::ocd) const BOTTOM: u16 = 0x00;
    pub(in crate::ocd) const MIDDLE: u16 = 0x04;
    pub(in crate::ocd) const TOP: u16 = 0x08;
}

/// How text is framed.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct Framing {
    pub(in crate::ocd) mode: u8,
    pub(in crate::ocd) line_style: u8,
    pub(in crate::ocd) point_symbol_on: u8,
    pub(in crate::ocd) point_symbol_number: u32,
    pub(in crate::ocd) reserved: [u8; 19],
    pub(in crate::ocd) border_left: u16,
    pub(in crate::ocd) border_bottom: u16,
    pub(in crate::ocd) border_right: u16,
    pub(in crate::ocd) border_top: u16,
    pub(in crate::ocd) color: u16,
    pub(in crate::ocd) line_width: u16,
    pub(in crate::ocd) font_weight: u16,
    pub(in crate::ocd) italic: u16,
    pub(in crate::ocd) offset_x: i16,
    pub(in crate::ocd) offset_y: i16,
}

impl Framing {
    pub(in crate::ocd) const NONE: u8 = 0;
    pub(in crate::ocd) const SHADOW: u8 = 1;
    pub(in crate::ocd) const LINE: u8 = 2;
}

/// What follows the base of a text symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct TextTail {
    pub(in crate::ocd) basic: BasicText,
    /// Percent of the font size.
    pub(in crate::ocd) line_spacing: u16,
    pub(in crate::ocd) paragraph_spacing: i16,
    pub(in crate::ocd) indent_first_line: u16,
    pub(in crate::ocd) indent_other_lines: u16,
    pub(in crate::ocd) tab_count: u16,
    pub(in crate::ocd) tabs: [i32; 32],
    pub(in crate::ocd) line_below_on: u16,
    pub(in crate::ocd) line_below_color: u16,
    pub(in crate::ocd) line_below_width: u16,
    pub(in crate::ocd) line_below_offset: u16,
    pub(in crate::ocd) reserved: u16,
    pub(in crate::ocd) framing: Framing,
}

/// What follows the base of a line text symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct LineTextTail {
    pub(in crate::ocd) basic: BasicText,
    pub(in crate::ocd) framing: Framing,
}

/// What follows the base of a rectangle symbol.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct RectangleTail {
    pub(in crate::ocd) line_color: u16,
    pub(in crate::ocd) line_width: u16,
    pub(in crate::ocd) corner_radius: u16,
    pub(in crate::ocd) grid_flags: u16,
}

impl RectangleTail {
    /// Grid flag: the rectangle has a grid.
    pub(in crate::ocd) const GRID_ON: u16 = 0x01;
}

/// A point symbol element's header; its coordinates follow it.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct ElementHeader {
    pub(in crate::ocd) element_type: u16,
    pub(in crate::ocd) flags: u16,
    pub(in crate::ocd) color: u16,
    pub(in crate::ocd) line_width: i16,
    pub(in crate::ocd) diameter: i16,
    pub(in crate::ocd) coord_count: u16,
    pub(in crate::ocd) reserved: [u8; 4],
}

impl ElementHeader {
    pub(in crate::ocd) const LINE: u16 = 1;
    pub(in crate::ocd) const AREA: u16 = 2;
    pub(in crate::ocd) const CIRCLE: u16 = 3;
    pub(in crate::ocd) const DOT: u16 = 4;

    /// Line flags: flat caps and bevel joins.
    pub(in crate::ocd) const FLAT_BEVEL: u16 = 0;
    /// Line flags: round caps and joins.
    pub(in crate::ocd) const ROUND: u16 = 1;
    /// Line flags: flat caps and miter joins.
    pub(in crate::ocd) const FLAT_MITER: u16 = 4;
}

/// A point symbol element: a dot, circle, line or area of one color.
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct Element {
    pub(in crate::ocd) header: ElementHeader,
    pub(in crate::ocd) coords: Vec<FileCoord>,
}

impl Element {
    /// Append the element to `bytes`.
    pub(in crate::ocd) fn write(&self, bytes: &mut Vec<u8>) -> BinResult<()> {
        let mut cursor = Cursor::new(bytes);
        cursor.seek(std::io::SeekFrom::End(0))?;
        self.header.write_le(&mut cursor)?;
        for coord in &self.coords {
            coord.write_le(&mut cursor)?;
        }
        Ok(())
    }
}

/// Read elements taking up `data_size` 8-byte units.
pub(in crate::ocd) fn read_elements<R: Read + Seek>(
    reader: &mut R,
    data_size: u16,
) -> BinResult<Vec<Element>> {
    let mut elements = Vec::new();
    let mut remaining = usize::from(data_size);
    while remaining >= 2 {
        let header = ElementHeader::read_le(reader)?;
        let count = usize::from(header.coord_count).min(remaining - 2);
        let coords = (0..count)
            .map(|_| FileCoord::read_le(reader))
            .collect::<BinResult<Vec<_>>>()?;
        remaining -= 2 + count;
        elements.push(Element { header, coords });
    }
    // Skip whatever a damaged size leaves over.
    reader.seek(std::io::SeekFrom::Current(
        i64::try_from(remaining * 8).unwrap_or(0),
    ))?;
    Ok(elements)
}
