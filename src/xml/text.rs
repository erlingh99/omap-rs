//! Element text content.

use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesEnd, BytesStart, BytesText, Event},
};

use crate::Result;

/// The text content of the element just started, up to its first child or end.
pub(crate) fn read_text<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<String> {
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
pub(crate) fn write_notes<W: std::io::Write>(notes: &str, writer: &mut Writer<W>) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("notes")))?;
    writer.write_event(Event::Text(BytesText::new(notes)))?;
    writer.write_event(Event::End(BytesEnd::new("notes")))?;
    Ok(())
}
