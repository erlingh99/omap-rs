//! Parameter strings: the tab-separated records that hold colors,
//! georeferencing, templates, notes and display settings.
//!
//! A record's first field runs up to the first tab. Each later field is a
//! one-character code followed by its value, up to the next tab.

use binrw::binrw;

use crate::ocd::file::{IndexEntry, OcdFile};
use crate::ocd::{Error, Result};

/// The record types this crate reads or writes.
pub(in crate::ocd) mod kind {
    /// A background map (template).
    pub(in crate::ocd) const TEMPLATE: i32 = 8;
    /// A color.
    pub(in crate::ocd) const COLOR: i32 = 9;
    /// A spot color separation.
    pub(in crate::ocd) const SPOT_COLOR: i32 = 10;
    /// The view: center, zoom and display modes.
    pub(in crate::ocd) const VIEW: i32 = 1030;
    /// Scale and georeferencing.
    pub(in crate::ocd) const SCALE: i32 = 1039;
    /// The map notes.
    pub(in crate::ocd) const NOTES: i32 = 1061;
}

/// A string index entry.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct StringIndexEntry {
    pub(in crate::ocd) position: u32,
    pub(in crate::ocd) size: u32,
    pub(in crate::ocd) kind: i32,
    pub(in crate::ocd) object_index: u32,
}

impl IndexEntry for StringIndexEntry {
    const SIZE: usize = 16;

    fn position(&self) -> u32 {
        self.position
    }

    fn set_position(&mut self, position: u32) {
        self.position = position;
    }
}

/// One parameter string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ocd) struct ParameterString {
    pub(in crate::ocd) kind: i32,
    pub(in crate::ocd) text: String,
}

impl ParameterString {
    /// A record of `kind` with `first` as its first field and nothing else.
    ///
    /// A tab in `first` would end the field and a NUL the record, so each is
    /// written as a space; so are they in the values of [`Self::push`]. A
    /// record of free text is made with [`Self::free_text`] instead.
    pub(in crate::ocd) fn new(kind: i32, first: &str) -> Self {
        Self {
            kind,
            text: without_separators(first),
        }
    }

    /// A record of `kind` that is all free text, with no fields: a tab in
    /// it is kept, and only a NUL, which would end the record, is written as
    /// a space.
    pub(in crate::ocd) fn free_text(kind: i32, text: &str) -> Self {
        Self {
            kind,
            text: text.replace('\0', " "),
        }
    }

    /// Append a field with `code` and `value`.
    pub(in crate::ocd) fn push(&mut self, code: char, value: impl std::fmt::Display) {
        use std::fmt::Write as _;
        // Writing to a `String` cannot fail.
        let value = without_separators(&value.to_string());
        let _infallible = write!(self.text, "\t{code}{value}");
    }

    /// The first field, up to the first tab.
    pub(in crate::ocd) fn first(&self) -> &str {
        self.text.split('\t').next().unwrap_or_default()
    }

    /// The coded fields after the first, in order, as code and value.
    pub(in crate::ocd) fn fields(&self) -> impl Iterator<Item = (char, &str)> {
        self.text.split('\t').skip(1).filter_map(|field| {
            let mut chars = field.chars();
            let code = chars.next()?;
            Some((code, chars.as_str()))
        })
    }
}

/// `text` with the tabs and NULs that end fields and records as spaces.
fn without_separators(text: &str) -> String {
    text.replace(['\t', '\0'], " ")
}

/// Every parameter string in `file` that is not deleted, in file order.
pub(in crate::ocd) fn read_strings(file: &OcdFile) -> Result<Vec<ParameterString>> {
    let entries: Vec<StringIndexEntry> = file.index(file.header().first_string_block)?;
    let mut strings = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.kind < 0 {
            continue;
        }
        let bytes = file.bytes_from(entry.position)?;
        let bytes = bytes.get(..entry.size as usize).unwrap_or(bytes);
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        strings.push(ParameterString {
            kind: entry.kind,
            text: String::from_utf8_lossy(&bytes[..end]).into_owned(),
        });
    }
    Ok(strings)
}

/// The index items for `strings`: each zero-terminated, as OCAD stores them.
pub(in crate::ocd) fn string_items(
    strings: &[ParameterString],
) -> Result<Vec<(StringIndexEntry, Vec<u8>)>> {
    strings
        .iter()
        .map(|string| {
            let mut bytes = string.text.as_bytes().to_vec();
            bytes.push(0);
            let size = u32::try_from(bytes.len()).map_err(|_too_long| Error::TooMany("text"))?;
            let entry = StringIndexEntry {
                size,
                kind: string.kind,
                ..StringIndexEntry::default()
            };
            Ok((entry, bytes))
        })
        .collect()
}

/// The value of a field as a number, if it is one.
pub(in crate::ocd) fn number(value: &str) -> Option<f64> {
    value.trim().parse().ok().filter(|v: &f64| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::ParameterString;

    #[test]
    fn tabs_and_nuls_in_written_text_do_not_start_fields_or_end_the_record() {
        let mut string = ParameterString::new(9, "Purple\tsZZZ");
        string.push('n', "1\0after");
        assert_eq!(string.first(), "Purple sZZZ");
        assert_eq!(string.fields().collect::<Vec<_>>(), [('n', "1 after")]);
    }

    #[test]
    fn free_text_keeps_its_tabs_and_only_a_nul_becomes_a_space() {
        let string = ParameterString::free_text(1061, "a\tb\0c");
        assert_eq!(string.text, "a\tb c");
    }

    #[test]
    fn fields_follow_the_first_field_and_skip_empty_ones() {
        let string = ParameterString {
            kind: 9,
            text: "Black\tn0\t\tc0\tk100".to_owned(),
        };
        assert_eq!(string.first(), "Black");
        assert_eq!(
            string.fields().collect::<Vec<_>>(),
            [('n', "0"), ('c', "0"), ('k', "100")]
        );
    }

    #[test]
    fn a_record_without_a_first_field_starts_with_a_tab() {
        let mut string = ParameterString::new(1039, "");
        string.push('m', 15000);
        string.push('a', 1.5);
        assert_eq!(string.text, "\tm15000\ta1.5");
        assert_eq!(string.first(), "");
        assert_eq!(
            string.fields().collect::<Vec<_>>(),
            [('m', "15000"), ('a', "1.5")]
        );
    }
}
