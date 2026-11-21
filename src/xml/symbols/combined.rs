//! Combined symbols and their parts.

use crate::colors::ColorSet;
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, CombinedAreaSymbol, CombinedSymbol, LineSymbol, PathSymbolId,
    PublicOrPrivateSymbol, SymbolCommon, SymbolId, SymbolSet,
};
use crate::xml::attrs::{parse_attr, parse_attr_raw, try_get_attr_raw};
use crate::xml::symbols::{SymbolPosition, UnresolvedPart};
use crate::{Error, OmapSection, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

pub(crate) trait PrivatePart {
    fn write_part<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()>;
}

impl PrivatePart for AreaOrLineSymbol {
    fn write_part<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()> {
        match self {
            Self::Area(symbol) => symbol.write(writer, color_set, SymbolPosition::Private),
            Self::Line(symbol) => symbol.write(writer, color_set, SymbolPosition::Private),
        }
    }
}

impl PrivatePart for LineSymbol {
    fn write_part<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()> {
        self.write(writer, color_set, SymbolPosition::Private)
    }
}

impl<Id: Copy + Into<SymbolId>, Private> CombinedSymbol<Id, Private> {
    pub(crate) fn write_body<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
        color_set: &ColorSet,
    ) -> Result<()>
    where
        Private: PrivatePart,
    {
        let mut cs = BytesStart::new("combined_symbol");
        cs.push_attribute(("parts", self.num_components().to_string().as_str()));
        writer.write_event(Event::Start(cs))?;

        for part in self.components() {
            match part {
                PublicOrPrivateSymbol::Public(id) => {
                    let sym_index = symbol_set.file_index(*id);
                    writer.write_event(Event::Empty(
                        BytesStart::new("part")
                            .with_attributes([("symbol", sym_index.to_string().as_str())]),
                    ))?;
                }
                PublicOrPrivateSymbol::Private(symbol) => {
                    writer.write_event(Event::Start(
                        BytesStart::new("part").with_attributes([("private", "true")]),
                    ))?;
                    symbol.write_part(writer, color_set)?;
                    writer.write_event(Event::End(BytesEnd::new("part")))?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("combined_symbol")))?;
        Ok(())
    }
}

impl CombinedAreaSymbol {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
        attributes: SymbolCommon,
    ) -> Result<(Self, Vec<UnresolvedPart>)> {
        let mut common = attributes;
        let mut parts: Vec<PublicOrPrivateSymbol<PathSymbolId, AreaOrLineSymbol>> = Vec::new();
        let mut unresolved: Vec<UnresolvedPart> = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    "description" => common.description = crate::xml::read_text(reader)?,
                    "combined_symbol" => {
                        // num_parts attribute is informational, we parse dynamically
                    }
                    "part" => {
                        let is_private = try_get_attr_raw(&e, "private")?.unwrap_or(false);
                        if is_private {
                            let sym = Self::parse_private_part(reader, color_set)?;
                            parts.push(PublicOrPrivateSymbol::Private(sym));
                        } else {
                            let symbol_index = try_get_attr_raw::<i32>(&e, "symbol")?;
                            // Mapper writes -1 for a part whose symbol is gone;
                            // it stays an empty part in its place.
                            if let Some(file_index) =
                                symbol_index.and_then(|index| usize::try_from(index).ok())
                            {
                                unresolved.push(UnresolvedPart {
                                    position: parts.len(),
                                    file_index,
                                });
                            }
                            parts.push(PublicOrPrivateSymbol::Public(None));
                        }
                    }
                    "icon" => common.custom_icon = try_get_attr_raw(&e, "src")?,
                    _ => {}
                },
                Event::End(e) if e.local_name().as_ref() == "symbol" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::CombinedAreaSymbol));
                }
                _ => {}
            }
        }

        Ok((Self::from_parts(common, parts), unresolved))
    }

    fn parse_private_part<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
    ) -> Result<AreaOrLineSymbol> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) if e.local_name().as_ref() == "symbol" => {
                    let sym_type: u8 = try_get_attr_raw(&e, "type")?.unwrap_or(0);
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
                    match sym_type {
                        2 => {
                            let line = LineSymbol::parse(reader, color_set, sub_common)?;
                            Self::skip_to_end_of_part(reader)?;
                            return Ok(AreaOrLineSymbol::Line(line));
                        }
                        4 => {
                            let area = AreaSymbol::parse(reader, color_set, sub_common)?;
                            Self::skip_to_end_of_part(reader)?;
                            return Ok(AreaOrLineSymbol::Area(area));
                        }
                        _ => {
                            return Err(Error::UnknownPrivatePartSymbolType(sym_type));
                        }
                    }
                }
                Event::End(e) if e.local_name().as_ref() == "part" => {
                    return Err(Error::EmptyPrivatePart);
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::PrivatePart));
                }
                _ => {}
            }
        }
    }

    fn skip_to_end_of_part<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<()> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::End(e) if e.local_name().as_ref() == "part" => {
                    return Ok(());
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::SkippedPart));
                }
                _ => {}
            }
        }
    }
}
