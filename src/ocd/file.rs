//! The file as a whole: its header, and the index blocks that list where its
//! parameter strings, symbols and objects lie.

use std::collections::HashSet;
use std::io::Cursor;

use binrw::{BinRead, BinWrite, binrw};

use crate::ocd::{Error, Result};

/// The mark every OCAD file starts with.
pub(in crate::ocd) const OCAD_MARK: u16 = 0x0cad;

/// The number of entries in one index block.
pub(in crate::ocd) const ENTRIES_PER_BLOCK: usize = 256;

/// The file header, 60 bytes at the start of the file.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct FileHeader {
    pub(in crate::ocd) mark: u16,
    pub(in crate::ocd) file_type: u8,
    pub(in crate::ocd) file_status: u8,
    pub(in crate::ocd) version: u16,
    pub(in crate::ocd) subversion: u8,
    pub(in crate::ocd) subsubversion: u8,
    pub(in crate::ocd) first_symbol_block: u32,
    pub(in crate::ocd) first_object_block: u32,
    pub(in crate::ocd) offline_sync_serial: u32,
    pub(in crate::ocd) current_file_version: u32,
    pub(in crate::ocd) internal: [u32; 2],
    pub(in crate::ocd) first_string_block: u32,
    pub(in crate::ocd) file_name_pos: u32,
    pub(in crate::ocd) file_name_size: u32,
    pub(in crate::ocd) internal_2: u32,
    pub(in crate::ocd) reserved: [u32; 2],
    pub(in crate::ocd) first_multi_rep_block: u32,
}

impl FileHeader {
    /// The header's size in the file.
    pub(in crate::ocd) const SIZE: usize = 60;

    /// The file versions this crate reads, which share one layout.
    pub(in crate::ocd) const VERSIONS: [u16; 2] = [12, 2018];

    /// File type: a map, the only kind this crate reads.
    pub(in crate::ocd) const MAP: u8 = 0;
}

/// An index entry: the fixed-size record an index block holds for each
/// parameter string, symbol or object.
pub(in crate::ocd) trait IndexEntry:
    for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()> + Default + Clone
{
    /// The entry's size in the file.
    const SIZE: usize;

    /// The file position of the entity this entry lists, 0 if unused.
    fn position(&self) -> u32;

    /// Point this entry at the entity at `position`.
    fn set_position(&mut self, position: u32);
}

/// A file read into memory, which entities are read from by file position.
pub(in crate::ocd) struct OcdFile {
    bytes: Vec<u8>,
    header: FileHeader,
}

impl OcdFile {
    /// Check the header of `bytes` and keep them for reading.
    pub(in crate::ocd) fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < FileHeader::SIZE {
            return Err(Error::NotOcad);
        }
        let header = FileHeader::read(&mut Cursor::new(&bytes))?;
        if header.mark != OCAD_MARK {
            return Err(Error::NotOcad);
        }
        if !FileHeader::VERSIONS.contains(&header.version) {
            return Err(Error::UnsupportedVersion(header.version));
        }
        if header.file_type != FileHeader::MAP {
            return Err(Error::NotAMap(header.file_type));
        }
        Ok(Self { bytes, header })
    }

    pub(in crate::ocd) fn header(&self) -> &FileHeader {
        &self.header
    }

    /// The bytes from `position` to the end of the file.
    pub(in crate::ocd) fn bytes_from(&self, position: u32) -> Result<&[u8]> {
        self.bytes
            .get(position as usize..)
            .filter(|rest| !rest.is_empty())
            .ok_or(Error::PositionOutOfBounds {
                position: position.into(),
                len: self.bytes.len(),
            })
    }

    /// Every used entry of the index whose first block is at `first_block`,
    /// in file order.
    ///
    /// An entry is used when it names a position inside the file. The chain
    /// ends at a next-block position of 0.
    pub(in crate::ocd) fn index<E: IndexEntry>(&self, first_block: u32) -> Result<Vec<E>> {
        let mut entries = Vec::new();
        let mut visited = HashSet::new();
        let mut block = first_block;
        while block != 0 {
            if !visited.insert(block) {
                return Err(Error::IndexLoop(block));
            }
            let mut cursor = Cursor::new(self.bytes_from(block)?);
            let next = u32::read_le(&mut cursor)?;
            for _ in 0..ENTRIES_PER_BLOCK {
                let entry = E::read_le(&mut cursor)?;
                let position = entry.position();
                if position != 0 && (position as usize) < self.bytes.len() {
                    entries.push(entry);
                }
            }
            block = next;
        }
        Ok(entries)
    }
}

/// Pad `bytes` with zeros to a multiple of 8, the alignment OCAD gives each
/// entity.
fn pad_to_8(bytes: &mut Vec<u8>) {
    let padded = bytes.len().next_multiple_of(8);
    bytes.resize(padded, 0);
}

/// A file being laid out: the header, then each index's blocks followed by
/// the entities they list.
pub(in crate::ocd) struct Layout {
    bytes: Vec<u8>,
}

impl Layout {
    /// An empty file with room for the header.
    pub(in crate::ocd) fn new() -> Self {
        Self {
            bytes: vec![0; FileHeader::SIZE],
        }
    }

    /// Add an index listing `items`, each an entry and the entity's bytes,
    /// and return the position of its first block.
    ///
    /// An empty index still gets one block, as OCAD writes it.
    pub(in crate::ocd) fn add_index<E: IndexEntry>(
        &mut self,
        items: Vec<(E, Vec<u8>)>,
    ) -> Result<u32> {
        let block_size = 4 + ENTRIES_PER_BLOCK * E::SIZE;
        let block_count = items.len().div_ceil(ENTRIES_PER_BLOCK).max(1);
        pad_to_8(&mut self.bytes);
        let first_block = self.bytes.len();
        self.bytes.resize(first_block + block_count * block_size, 0);

        let mut entries = Vec::with_capacity(items.len());
        for (mut entry, entity) in items {
            pad_to_8(&mut self.bytes);
            entry.set_position(file_position(self.bytes.len())?);
            self.bytes.extend_from_slice(&entity);
            entries.push(entry);
        }

        let mut entries = entries.into_iter();
        for block in 0..block_count {
            let start = first_block + block * block_size;
            let next = if block + 1 < block_count {
                file_position(start + block_size)?
            } else {
                0
            };
            let mut cursor = Cursor::new(&mut self.bytes[start..start + block_size]);
            next.write_le(&mut cursor)?;
            for entry in entries.by_ref().take(ENTRIES_PER_BLOCK) {
                entry.write_le(&mut cursor)?;
            }
        }
        file_position(first_block)
    }

    /// The finished file, with `header` at its start.
    pub(in crate::ocd) fn finish(mut self, header: &FileHeader) -> Result<Vec<u8>> {
        header.write_le(&mut Cursor::new(&mut self.bytes[..FileHeader::SIZE]))?;
        Ok(self.bytes)
    }
}

/// `position` as a file position, which the format stores in 32 bits.
fn file_position(position: usize) -> Result<u32> {
    u32::try_from(position).map_err(|_too_large| Error::TooMany("bytes"))
}
