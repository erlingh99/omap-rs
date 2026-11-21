//! The `<parts>` section: map parts and their objects.

use crate::objects::MapObject;
use crate::parts::{MapPart, MapParts};
use crate::symbols::SymbolSet;
use crate::xml::attrs::try_get_attr;
use crate::xml::{Error, Result, Section};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

impl MapPart {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        symbols: &SymbolSet,
    ) -> Result<Self> {
        let name = try_get_attr(element, "name")
            .ok()
            .flatten()
            .unwrap_or(String::new());

        let mut objects = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "object") {
                        let object = MapObject::parse(reader, &bytes_start, symbols, false)?;
                        if object.geometry_is_empty() {
                            continue;
                        }
                        objects.push(object);
                    }
                }
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "part") {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::MapPart));
                }
                _ => (),
            }
        }

        Ok(Self {
            name,
            objects: objects.into_iter().collect(),
        })
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbols: &SymbolSet,
    ) -> Result<()> {
        let object_count = self
            .objects
            .values()
            .filter(|object| !object.geometry_is_empty())
            .count();

        writer.write_event(Event::Start(
            BytesStart::new("part").with_attributes([("name", self.name.as_str())]),
        ))?;
        writer.write_event(Event::Start(
            BytesStart::new("objects")
                .with_attributes([("count", object_count.to_string().as_str())]),
        ))?;
        writer.get_mut().write_all(b"\n")?;
        for object in self.objects.values() {
            if object.geometry_is_empty() {
                continue;
            }
            object.write(writer, symbols)?;
            writer.get_mut().write_all(b"\n")?;
        }
        writer.write_event(Event::End(BytesEnd::new("objects")))?;
        writer.write_event(Event::End(BytesEnd::new("part")))?;
        Ok(())
    }
}

impl MapParts {
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbols: &SymbolSet,
    ) -> Result<()> {
        writer.write_event(Event::Start(BytesStart::new("parts").with_attributes([
            ("count", self.len().to_string().as_str()),
            ("current", "0"),
        ])))?;
        writer.get_mut().write_all(b"\n")?;

        for part in self.values() {
            part.write(writer, symbols)?;
            writer.get_mut().write_all(b"\n")?;
        }

        writer.write_event(Event::End(BytesEnd::new("parts")))?;
        Ok(())
    }

    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbols: &SymbolSet,
    ) -> Result<Self> {
        let mut parts = Self::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "part") {
                        let _id = parts.push(MapPart::parse(reader, &bytes_start, symbols)?);
                    }
                }
                Event::End(_) => break,
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Parts));
                }
                _ => (),
            }
        }

        Ok(parts)
    }
}
#[cfg(test)]
mod tests {

    use geo_types::{LineString, coord};
    use quick_xml::{Reader, Writer, events::Event};

    use super::MapPart;
    use crate::objects::LineObject;
    use crate::objects::MapObject;
    use crate::symbols::SymbolSet;
    use crate::xml::Result;

    fn empty_symbol_set() -> SymbolSet {
        SymbolSet::new()
    }

    #[test]
    fn parsed_objects_without_geometry_are_ignored() -> Result<()> {
        let mut reader = Reader::from_str(
            r#"<part name="Map"><objects count="1"><object type="1"><coords count="0"></coords></object></objects></part>"#,
        );
        let Event::Start(start) = reader.read_event()? else {
            panic!("expected part start");
        };

        let part = MapPart::parse(&mut reader, &start, &empty_symbol_set())?;

        assert!(part.objects.is_empty());
        Ok(())
    }

    #[test]
    fn empty_objects_are_retained_in_memory_and_skipped_on_write() -> Result<()> {
        let symbol = None;
        let mut part = MapPart::new("Map");
        let _id = part
            .objects
            .push(LineObject::new(symbol, LineString::new(Vec::new())));
        assert_eq!(part.objects.len(), 1);

        let _id = part.objects.push(LineObject::new(
            symbol,
            LineString::new(vec![coord! { x: 0., y: 0. }, coord! { x: 1., y: 0. }]),
        ));
        let Some(MapObject::Line(line)) = part.objects.values_mut().nth(1) else {
            panic!("expected line object");
        };
        *line.geometry_mut() = crate::objects::BezierPath::empty();
        assert_eq!(part.objects.len(), 2);

        let mut writer = Writer::new(Vec::new());
        part.write(&mut writer, &empty_symbol_set())?;
        let output = String::from_utf8(writer.into_inner())?;

        assert!(output.contains(r#"<objects count="0">"#));
        assert!(!output.contains("<object "));
        Ok(())
    }
}
