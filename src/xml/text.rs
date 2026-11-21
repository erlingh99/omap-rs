//! Element text content.

use crate::xml::Result;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};

/// The text content of the element just started, up to its first child or end.
pub(in crate::xml) fn read_text<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<String> {
    let mut notes = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Text(bytes_text) => {
                notes.push_str(&bytes_text.xml_content(XmlVersion::Explicit1_0));
            }
            Event::GeneralRef(bytes_ref) => {
                notes.push_str(&quick_xml::escape::unescape(&format!(
                    "&{};",
                    bytes_ref.xml_content(XmlVersion::Explicit1_0)
                ))?);
            }
            _ => break,
        }
    }
    Ok(notes)
}

/// A `<notes>` element holding `notes`.
pub(in crate::xml) fn write_notes<W: std::io::Write>(
    notes: &str,
    writer: &mut Writer<W>,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("notes")))?;
    writer.write_event(Event::Text(BytesText::new(notes)))?;
    writer.write_event(Event::End(BytesEnd::new("notes")))?;
    Ok(())
}
