//! Line symbols.

use crate::colors::{ColorSet, SymbolColor};
use crate::symbols::{
    BorderDash, BorderStyle, CapStyle, DashStyle, DashSymbol, GroupDashes, JoinStyle, LineSymbol,
    LineSymbolBorder, MidSymbol, MidSymbolPlacement, PointSymbol, SymbolCommon, SymbolKind,
};
use crate::xml::attrs::{parse_attr, parse_attr_raw, try_get_attr_raw};
use crate::xml::symbols::SymbolPosition;
use crate::{Error, NonNegativeF64, OmapSection, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::str::FromStr;

impl FromStr for CapStyle {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "0" => Ok(Self::Flat),
            "1" => Ok(Self::Round),
            "2" => Ok(Self::Square),
            "3" => Ok(Self::Pointed),
            _ => Err(Error::UnknownCapStyle),
        }
    }
}

impl FromStr for JoinStyle {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "0" => Ok(Self::Bevel),
            "1" => Ok(Self::Miter),
            "2" => Ok(Self::Round),
            _ => Err(Error::UnknownJoinStyle),
        }
    }
}

impl FromStr for MidSymbolPlacement {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "0" => Ok(Self::CenterOfDash),
            "1" => Ok(Self::CenterOfDashGroup),
            "2" => Ok(Self::CenterOfGap),
            _ => Err(Error::UnknownMidSymbolPlacement),
        }
    }
}

impl LineSymbolBorder {
    fn parse(element: &BytesStart<'_>, color_set: &ColorSet) -> Result<Self> {
        let color_index = try_get_attr_raw(element, "color")?.unwrap_or(-1);
        let color = SymbolColor::from_index(color_index, color_set);
        let width =
            NonNegativeF64::from_file_value(try_get_attr_raw(element, "width")?.unwrap_or(0));
        let shift =
            NonNegativeF64::from_file_value(try_get_attr_raw(element, "shift")?.unwrap_or(0));
        let is_dashed = try_get_attr_raw(element, "dashed")?.unwrap_or(false);
        let dashed = if is_dashed {
            Some(BorderDash {
                dash_length: NonNegativeF64::from_file_value(
                    try_get_attr_raw(element, "dash_length")?.unwrap_or(0),
                ),
                break_length: NonNegativeF64::from_file_value(
                    try_get_attr_raw(element, "break_length")?.unwrap_or(0),
                ),
            })
        } else {
            None
        };
        Ok(Self {
            color,
            width,
            shift,
            dashed,
        })
    }

    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>, color_set: &ColorSet) -> Result<()> {
        let mut bs = BytesStart::new("border").with_attributes([
            ("color", self.color.priority(color_set).to_string().as_str()),
            ("width", self.width.to_file_value()?.to_string().as_str()),
            ("shift", self.shift.to_file_value()?.to_string().as_str()),
        ]);
        if let Some(border_dash) = &self.dashed {
            bs.push_attribute(("dashed", "true"));
            bs.push_attribute((
                "break_length",
                border_dash
                    .break_length
                    .to_file_value()?
                    .to_string()
                    .as_str(),
            ));
            bs.push_attribute((
                "dash_length",
                border_dash
                    .dash_length
                    .to_file_value()?
                    .to_string()
                    .as_str(),
            ));
        }
        writer.write_event(Event::Empty(bs))?;
        Ok(())
    }
}

impl LineSymbol {
    #[expect(
        clippy::too_many_lines,
        reason = "line-symbol parsing maps a large file-format record"
    )]
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
        attributes: SymbolCommon,
    ) -> Result<Self> {
        let mut common = attributes;
        let mut color = SymbolColor::NoColor;
        let mut line_width = NonNegativeF64::default();
        let mut minimum_length = NonNegativeF64::default();
        let mut start_offset = NonNegativeF64::default();
        let mut end_offset = NonNegativeF64::default();
        let mut cap_style = CapStyle::default();
        let mut join_style = JoinStyle::default();
        let mut dashed = false;
        let mut segment_length = 4000;
        let mut end_length = 0;
        let mut dash_length = 4000;
        let mut break_length = 1000;
        let mut dashes_in_group = 1;
        let mut in_group_break_length = 500;
        let mut half_outer_dashes = false;
        let mut show_at_least_one_symbol = true;
        let mut minimum_mid_symbol_count = 0;
        let mut minimum_mid_symbol_count_when_closed = 0;
        let mut mid_symbols_per_spot = 1;
        let mut mid_symbol_distance = 0;
        let mut mid_symbol_placement = MidSymbolPlacement::default();
        let mut suppress_dash_symbol_at_ends = false;
        let mut scale_dash_symbol = true;

        let mut start_symbol = None;
        let mut mid_symbol_point = None;
        let mut end_symbol = None;
        let mut dash_symbol_point = None;
        let mut border = None;

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    "description" => common.description = crate::xml::read_text(reader)?,
                    "line_symbol" => {
                        let color_index = try_get_attr_raw(&e, "color")?.unwrap_or(-1);
                        color = SymbolColor::from_index(color_index, color_set);
                        line_width = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "line_width")?.unwrap_or(0),
                        );
                        minimum_length = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "minimum_length")?.unwrap_or(0),
                        );
                        join_style = try_get_attr_raw(&e, "join_style")?.unwrap_or_default();
                        cap_style = try_get_attr_raw(&e, "cap_style")?.unwrap_or_default();
                        start_offset = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "start_offset")?.unwrap_or(0),
                        );
                        end_offset = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "end_offset")?.unwrap_or(0),
                        );
                        dashed = try_get_attr_raw(&e, "dashed")?.unwrap_or(false);
                        segment_length = try_get_attr_raw(&e, "segment_length")?.unwrap_or(4000);
                        end_length = try_get_attr_raw(&e, "end_length")?.unwrap_or(0);
                        dash_length = try_get_attr_raw(&e, "dash_length")?.unwrap_or(4000);
                        break_length = try_get_attr_raw(&e, "break_length")?.unwrap_or(1000);
                        dashes_in_group = try_get_attr_raw(&e, "dashes_in_group")?.unwrap_or(1);
                        in_group_break_length =
                            try_get_attr_raw(&e, "in_group_break_length")?.unwrap_or(500);
                        half_outer_dashes =
                            try_get_attr_raw(&e, "half_outer_dashes")?.unwrap_or(false);
                        show_at_least_one_symbol =
                            try_get_attr_raw(&e, "show_at_least_one_symbol")?.unwrap_or(false);
                        minimum_mid_symbol_count =
                            try_get_attr_raw(&e, "minimum_mid_symbol_count")?.unwrap_or(0);
                        minimum_mid_symbol_count_when_closed =
                            try_get_attr_raw(&e, "minimum_mid_symbol_count_when_closed")?
                                .unwrap_or(0);
                        mid_symbols_per_spot =
                            try_get_attr_raw(&e, "mid_symbols_per_spot")?.unwrap_or(1);
                        mid_symbol_distance =
                            try_get_attr_raw(&e, "mid_symbol_distance")?.unwrap_or(0);
                        mid_symbol_placement =
                            try_get_attr_raw(&e, "mid_symbol_placement")?.unwrap_or_default();
                        suppress_dash_symbol_at_ends =
                            try_get_attr_raw(&e, "suppress_dash_symbol_at_ends")?.unwrap_or(false);
                        scale_dash_symbol =
                            try_get_attr_raw(&e, "scale_dash_symbol")?.unwrap_or(true);
                    }
                    "start_symbol" => {
                        start_symbol = Self::parse_sub_point_symbol(reader, color_set)?;
                    }
                    "mid_symbol" => {
                        mid_symbol_point = Self::parse_sub_point_symbol(reader, color_set)?;
                    }
                    "end_symbol" => {
                        end_symbol = Self::parse_sub_point_symbol(reader, color_set)?;
                    }
                    "dash_symbol" => {
                        dash_symbol_point = Self::parse_sub_point_symbol(reader, color_set)?;
                    }
                    "borders" => {
                        border = Self::parse_borders(reader, &e, color_set)?;
                    }
                    "icon" => common.custom_icon = try_get_attr_raw(&e, "src")?,
                    _ => {}
                },
                Event::End(e) if e.local_name().as_ref() == "symbol" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::LineSymbol));
                }
                _ => {}
            }
        }

        let dash_style = if dashed {
            let dash_group = if dashes_in_group >= 2 {
                GroupDashes::Grouped {
                    dashes_in_group,
                    in_group_break_length: NonNegativeF64::from_file_value(in_group_break_length),
                }
            } else {
                GroupDashes::UnGrouped { half_outer_dashes }
            };
            DashStyle::Dashed {
                dash_length: NonNegativeF64::from_file_value(dash_length),
                break_length: NonNegativeF64::from_file_value(break_length),
                dash_group,
            }
        } else {
            DashStyle::NotDashed {
                segment_length: NonNegativeF64::from_file_value(segment_length),
                end_length: NonNegativeF64::from_file_value(end_length),
            }
        };

        let mid_symbol = mid_symbol_point
            .map(|ps| MidSymbol {
                mid_symbols_per_spot,
                mid_symbol_distance: NonNegativeF64::from_file_value(mid_symbol_distance),
                minimum_mid_symbol_count,
                minimum_mid_symbol_count_when_closed,
                show_at_least_one_mid_symbol: show_at_least_one_symbol,
                mid_symbol_placement,
                mid_symbol: ps,
            })
            .map(Box::new);

        let dash_symbol = dash_symbol_point
            .map(|ps| DashSymbol {
                suppress_dash_symbol_at_ends,
                scale_dash_symbol,
                dash_symbol: ps,
            })
            .map(Box::new);

        Ok(Self {
            common,
            border,
            start_symbol: start_symbol.map(Box::new),
            mid_symbol,
            end_symbol: end_symbol.map(Box::new),
            dash_symbol,
            color,
            line_width,
            minimum_length,
            start_offset,
            end_offset,
            dash_style,
            cap_style,
            join_style,
        })
    }

    /// Parse a sub point symbol (start/mid/end/dash) wrapped in its container element
    fn parse_sub_point_symbol<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
    ) -> Result<Option<PointSymbol>> {
        let mut buf = Vec::new();
        let mut result = None;
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) if e.local_name().as_ref() == "symbol" => {
                    let mut sub_common = SymbolCommon::default();
                    for attr in e.attributes().filter_map(std::result::Result::ok) {
                        match attr.key.local_name().as_ref() {
                            "name" => {
                                sub_common.name = parse_attr(attr).unwrap_or(sub_common.name);
                            }
                            "code" => {
                                sub_common.code = parse_attr_raw(attr.value).unwrap_or_default();
                            }
                            _ => {}
                        }
                    }
                    result = Some(PointSymbol::parse(reader, color_set, sub_common)?);
                }
                Event::End(e) => match e.local_name().as_ref() {
                    "start_symbol" | "mid_symbol" | "end_symbol" | "dash_symbol" => break,
                    _ => (),
                },
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::PointSymbol));
                }
                _ => {}
            }
        }
        if let Some(ps) = &result
            && ps.inner_color == SymbolColor::NoColor
            && ps.outer_color == SymbolColor::NoColor
            && ps.elements.is_empty()
        {
            return Ok(None);
        }
        Ok(result)
    }

    /// Parse borders element
    fn parse_borders<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        color_set: &ColorSet,
    ) -> Result<Option<BorderStyle>> {
        let borders_different = try_get_attr_raw(element, "borders_different")?.unwrap_or(false);
        let mut left = None;
        let mut right = None;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) if e.local_name().as_ref() == "border" => {
                    let b = LineSymbolBorder::parse(&e, color_set)?;
                    if left.is_none() {
                        left = Some(b);
                    } else {
                        right = Some(b);
                    }
                }
                Event::End(e) if e.local_name().as_ref() == "borders" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::LineSymbol));
                }
                _ => {}
            }
        }
        match (left, right) {
            (Some(l), Some(r)) if borders_different => {
                Ok(Some(BorderStyle::AsymmetricBorder { left: l, right: r }))
            }
            (Some(both), _) => Ok(Some(BorderStyle::SymmetricBorder { both })),
            _ => Ok(None),
        }
    }

    /// Write this symbol on its own, for the sub-symbol positions that
    /// [`Symbol::write`] does not reach: private parts of a combined symbol,
    /// and point-symbol elements.
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
        position: SymbolPosition,
    ) -> Result<()> {
        self.common
            .write_open(writer, SymbolKind::Line.type_id(), position)?;
        self.write_body(writer, color_set)?;
        self.common.write_close(writer)
    }

    /// Write the type-specific body, between the halves of the shared
    /// `<symbol>` frame written by [`Symbol::write`].
    #[expect(
        clippy::too_many_lines,
        reason = "line-symbol serialization maps a large file-format record"
    )]
    pub(crate) fn write_body<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()> {
        let main_color = self.color.priority(color_set);

        let mut bs = BytesStart::new("line_symbol").with_attributes([
            ("color", main_color.to_string().as_str()),
            (
                "line_width",
                self.line_width.to_file_value()?.to_string().as_str(),
            ),
            (
                "minimum_length",
                self.minimum_length.to_file_value()?.to_string().as_str(),
            ),
            ("join_style", (self.join_style as u8).to_string().as_str()),
            ("cap_style", (self.cap_style as u8).to_string().as_str()),
            (
                "start_offset",
                self.start_offset.to_file_value()?.to_string().as_str(),
            ),
            (
                "end_offset",
                self.end_offset.to_file_value()?.to_string().as_str(),
            ),
        ]);

        match &self.dash_style {
            DashStyle::Dashed {
                dash_length,
                break_length,
                dash_group,
            } => {
                bs.push_attribute(("dashed", "true"));
                bs.push_attribute((
                    "dash_length",
                    dash_length.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "break_length",
                    break_length.to_file_value()?.to_string().as_str(),
                ));
                match dash_group {
                    GroupDashes::Grouped {
                        dashes_in_group,
                        in_group_break_length,
                    } => {
                        bs.push_attribute((
                            "dashes_in_group",
                            dashes_in_group.to_string().as_str(),
                        ));
                        bs.push_attribute((
                            "in_group_break_length",
                            in_group_break_length.to_file_value()?.to_string().as_str(),
                        ));
                    }
                    GroupDashes::UnGrouped { half_outer_dashes } => {
                        bs.push_attribute(("dashes_in_group", "1"));
                        if *half_outer_dashes {
                            bs.push_attribute(("half_outer_dashes", "true"));
                        }
                    }
                }
            }
            DashStyle::NotDashed {
                segment_length,
                end_length,
            } => {
                bs.push_attribute((
                    "segment_length",
                    segment_length.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "end_length",
                    end_length.to_file_value()?.to_string().as_str(),
                ));
            }
        }
        if let Some(mid_symbol) = &self.mid_symbol {
            bs.push_attribute((
                "mid_symbol_distance",
                mid_symbol
                    .mid_symbol_distance
                    .to_file_value()?
                    .to_string()
                    .as_str(),
            ));
            bs.push_attribute((
                "mid_symbol_placement",
                (mid_symbol.mid_symbol_placement as u8).to_string().as_str(),
            ));
            bs.push_attribute((
                "mid_symbols_per_spot",
                mid_symbol.mid_symbols_per_spot.to_string().as_str(),
            ));
            bs.push_attribute((
                "minimum_mid_symbol_count",
                mid_symbol.minimum_mid_symbol_count.to_string().as_str(),
            ));
            bs.push_attribute((
                "minimum_mid_symbol_count_when_closed",
                mid_symbol
                    .minimum_mid_symbol_count_when_closed
                    .to_string()
                    .as_str(),
            ));
            if mid_symbol.show_at_least_one_mid_symbol {
                bs.push_attribute(("show_at_least_one_symbol", "true"));
            }
        }
        if let Some(dash_symbol) = &self.dash_symbol {
            bs.push_attribute((
                "scale_dash_symbol",
                dash_symbol.scale_dash_symbol.to_string().as_str(),
            ));
            if dash_symbol.suppress_dash_symbol_at_ends {
                bs.push_attribute(("suppress_dash_symbol_at_ends", "true"));
            }
        }
        writer.write_event(Event::Start(bs))?;

        if let Some(border) = &self.border {
            match border {
                BorderStyle::SymmetricBorder { both } => {
                    writer.write_event(Event::Start(BytesStart::new("borders")))?;
                    both.write(writer, color_set)?;
                    writer.write_event(Event::End(BytesEnd::new("borders")))?;
                }
                BorderStyle::AsymmetricBorder { left, right } => {
                    writer.write_event(Event::Start(
                        BytesStart::new("borders").with_attributes([("borders_different", "true")]),
                    ))?;
                    left.write(writer, color_set)?;
                    right.write(writer, color_set)?;
                    writer.write_event(Event::End(BytesEnd::new("borders")))?;
                }
            }
        }
        if let Some(start_symbol) = &self.start_symbol {
            writer.write_event(Event::Start(BytesStart::new("start_symbol")))?;
            start_symbol.write(writer, color_set, SymbolPosition::Element)?;
            writer.write_event(Event::End(BytesEnd::new("start_symbol")))?;
        }
        if let Some(mid_symbol) = &self.mid_symbol {
            writer.write_event(Event::Start(BytesStart::new("mid_symbol")))?;
            mid_symbol
                .mid_symbol
                .write(writer, color_set, SymbolPosition::Element)?;
            writer.write_event(Event::End(BytesEnd::new("mid_symbol")))?;
        }
        if let Some(dash_symbol) = &self.dash_symbol {
            writer.write_event(Event::Start(BytesStart::new("dash_symbol")))?;
            dash_symbol
                .dash_symbol
                .write(writer, color_set, SymbolPosition::Element)?;
            writer.write_event(Event::End(BytesEnd::new("dash_symbol")))?;
        }
        if let Some(end_symbol) = &self.end_symbol {
            writer.write_event(Event::Start(BytesStart::new("end_symbol")))?;
            end_symbol.write(writer, color_set, SymbolPosition::Element)?;
            writer.write_event(Event::End(BytesEnd::new("end_symbol")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("line_symbol")))?;

        Ok(())
    }
}
