//! Point symbols and their elements.

use crate::NonNegativeF64;
use crate::colors::{ColorSet, SymbolColor};
use crate::objects::{AreaObject, LineObject, PointObject};
use crate::symbols::{AreaSymbol, Element, LineSymbol, PointSymbol, SymbolCommon, SymbolKind};
use crate::xml::attrs::try_get_attr_raw;
use crate::xml::symbols::SymbolPosition;
use crate::xml::{Error, Result, Section};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

/// Temporary enum used during element parsing
enum ElementSymbolData {
    Point(Box<PointSymbol>),
    Line(Box<LineSymbol>),
    Area(Box<AreaSymbol>),
}

/// Temporary enum used during element parsing
enum ElementObjectData {
    Point(Box<PointObject>),
    Line(Box<LineObject>),
    Area(Box<AreaObject>),
}

impl Element {
    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>, color_set: &ColorSet) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        writer.write_event(Event::Start(BytesStart::new("element")))?;
        match self {
            Self::Point { symbol, object } => {
                symbol.write(writer, color_set, SymbolPosition::Element)?;
                object.write_as_element(writer, symbol.is_rotatable)?;
            }
            Self::Line { symbol, object } => {
                symbol.write(writer, color_set, SymbolPosition::Element)?;
                object.write_as_element(writer)?;
            }
            Self::Area { symbol, object } => {
                symbol.write(writer, color_set, SymbolPosition::Element)?;
                object.write_as_element(writer)?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("element")))?;
        Ok(())
    }

    /// Parse a single element inside `point_symbol`
    fn parse_element<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
    ) -> Result<Self> {
        let mut symbol_data = None;
        let mut object_data = None;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    "symbol" => {
                        let sym_type = try_get_attr_raw(&e, "type")?.unwrap_or(0_u8);
                        symbol_data = Some(match sym_type {
                            1 => ElementSymbolData::Point(Box::new(PointSymbol::parse(
                                reader,
                                color_set,
                                Default::default(),
                            )?)),
                            2 => ElementSymbolData::Line(Box::new(LineSymbol::parse(
                                reader,
                                color_set,
                                Default::default(),
                            )?)),
                            4 => ElementSymbolData::Area(Box::new(AreaSymbol::parse(
                                reader,
                                color_set,
                                Default::default(),
                            )?)),
                            _ => return Err(Error::UnknownElementSymbolType(sym_type)),
                        });
                    }
                    "object" => {
                        let obj_type = try_get_attr_raw(&e, "type")?.unwrap_or(6_u8);
                        object_data = Some(match obj_type {
                            0 => ElementObjectData::Point(Box::new(PointObject::parse(
                                reader, None, 0.,
                            )?)),
                            1 => match &symbol_data {
                                Some(s) => match s {
                                    ElementSymbolData::Line(_) => ElementObjectData::Line(
                                        Box::new(LineObject::parse(reader, None)?),
                                    ),
                                    ElementSymbolData::Area(_) => ElementObjectData::Area(
                                        Box::new(AreaObject::parse(reader, None)?),
                                    ),
                                    ElementSymbolData::Point(_) => {
                                        return Err(Error::ElementSymbolObjectMismatch);
                                    }
                                },
                                None => return Err(Error::ElementObjectBeforeSymbol),
                            },
                            _ => return Err(Error::UnknownElementObjectType(obj_type)),
                        });
                    }
                    _ => {}
                },
                Event::End(e) if e.local_name().as_ref() == "element" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Element));
                }
                _ => {}
            }
        }
        if let Some(sd) = symbol_data
            && let Some(od) = object_data
        {
            match (sd, od) {
                (
                    ElementSymbolData::Point(point_symbol),
                    ElementObjectData::Point(point_object),
                ) => {
                    return Ok(Self::Point {
                        symbol: point_symbol,
                        object: point_object,
                    });
                }
                (ElementSymbolData::Line(line_symbol), ElementObjectData::Line(line_object)) => {
                    return Ok(Self::Line {
                        symbol: line_symbol,
                        object: line_object,
                    });
                }
                (ElementSymbolData::Area(area_symbol), ElementObjectData::Area(area_object)) => {
                    return Ok(Self::Area {
                        symbol: area_symbol,
                        object: area_object,
                    });
                }
                _ => {
                    return Err(Error::ElementSymbolObjectMismatch);
                }
            }
        }
        Err(Error::MissingElementData)
    }
}

impl PointSymbol {
    pub(in crate::xml) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
        mut common: SymbolCommon,
    ) -> Result<Self> {
        let mut is_rotatable = false;
        let mut inner_radius = NonNegativeF64::default();
        let mut inner_color = SymbolColor::NoColor;
        let mut outer_width = NonNegativeF64::default();
        let mut outer_color = SymbolColor::NoColor;
        let mut elements = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    "description" => common.description = crate::xml::read_text(reader)?,
                    "point_symbol" => {
                        is_rotatable = try_get_attr_raw(&e, "rotatable")?.unwrap_or(is_rotatable);
                        inner_radius = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "inner_radius")?.unwrap_or(0),
                        );
                        inner_color = SymbolColor::from_index(
                            try_get_attr_raw(&e, "inner_color")?.unwrap_or(-1),
                            color_set,
                        );
                        outer_width = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "outer_width")?.unwrap_or(0),
                        );
                        outer_color = SymbolColor::from_index(
                            try_get_attr_raw(&e, "outer_color")?.unwrap_or(-1),
                            color_set,
                        );
                    }
                    "element" => elements.push(Element::parse_element(reader, color_set)?),
                    "icon" => common.custom_icon = try_get_attr_raw(&e, "src")?,
                    _ => {}
                },
                Event::End(e) if e.local_name().as_ref() == "symbol" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::PointSymbol));
                }
                _ => {}
            }
        }

        elements.retain(|element| !element.is_empty());

        Ok(Self {
            common,
            is_rotatable,
            elements,
            inner_color,
            outer_color,
            inner_radius,
            outer_width,
        })
    }

    /// Write this symbol on its own, for the sub-symbol positions that
    /// [`Symbol::write`] does not reach: private parts of a combined symbol,
    /// and point-symbol elements.
    pub(in crate::xml) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
        position: SymbolPosition,
    ) -> Result<()> {
        self.common
            .write_open(writer, SymbolKind::Point.type_id(), position)?;
        self.write_body(writer, color_set)?;
        self.common.write_close(writer)
    }

    /// Write the type-specific body, between the halves of the shared
    /// `<symbol>` frame written by [`Symbol::write`].
    pub(in crate::xml) fn write_body<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()> {
        let mut bs = BytesStart::new("point_symbol");
        if self.is_rotatable {
            bs.push_attribute(("rotatable", "true"));
        }
        bs.push_attribute((
            "inner_radius",
            self.inner_radius.to_file_value()?.to_string().as_str(),
        ));
        bs.push_attribute((
            "inner_color",
            self.inner_color.priority(color_set).to_string().as_str(),
        ));
        bs.push_attribute((
            "outer_width",
            self.outer_width.to_file_value()?.to_string().as_str(),
        ));
        bs.push_attribute((
            "outer_color",
            self.outer_color.priority(color_set).to_string().as_str(),
        ));
        let element_count = self
            .elements
            .iter()
            .filter(|element| !element.is_empty())
            .count();
        bs.push_attribute(("elements", element_count.to_string().as_str()));
        writer.write_event(Event::Start(bs))?;

        for element in self.elements.iter().filter(|element| !element.is_empty()) {
            element.write(writer, color_set)?;
        }

        writer.write_event(Event::End(BytesEnd::new("point_symbol")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use geo_types::LineString;
    use quick_xml::Writer;

    use crate::Code;
    use crate::colors::ColorSet;
    use crate::objects::LineObject;
    use crate::symbols::LineSymbol;
    use crate::symbols::{Element, PointSymbol};
    use crate::xml::Result;
    use crate::xml::symbols::SymbolPosition;

    fn empty_line_element() -> Element {
        Element::Line {
            symbol: Box::new(LineSymbol::new(Code::default(), "")),
            object: Box::new(LineObject::new_element(LineString::new(Vec::new()))),
        }
    }

    #[test]
    fn empty_path_elements_are_rejected_and_skipped_on_write() -> Result<()> {
        let symbol = PointSymbol::new(Code::default(), "").with_element(empty_line_element());
        assert!(symbol.elements.is_empty());

        let mut symbol = PointSymbol::new(Code::default(), "");
        symbol.elements.push(empty_line_element());
        let mut writer = Writer::new(Vec::new());
        symbol.write(&mut writer, &ColorSet::default(), SymbolPosition::Private)?;
        let output = String::from_utf8(writer.into_inner())?;

        assert!(output.contains(r#"elements="0""#));
        assert!(!output.contains("<element>"));
        Ok(())
    }
}
