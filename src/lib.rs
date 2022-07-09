#![allow(unused)]
use byteorder::{NativeEndian, ReadBytesExt};
use object::read::elf::{FileHeader, ProgramHeader};
use object::{elf, Endianness};
use sentry_backtrace::{Frame, Stacktrace};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::fs;
use std::io::{BufRead, Cursor};
use std::path::Path;

#[derive(Debug)]
enum CoredumpError {
    FileFormatNotSupported,
    MissingDataSection,
    SymbolizationFailed,
    NotCoreFile,
}
impl Display for CoredumpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for CoredumpError {}

#[derive(Debug)]
struct Stackframe {
    start: u64,
    end: u64,
    offset: u64,
    name: String,
}
impl Display for Stackframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Start: {:x} End: {:x} Offset: {:x} Name: {}",
            self.start, self.end, self.offset, self.name
        )
    }
}

#[derive(Debug)]
struct CoredumpNotesHeader {
    page_size: u64,
    frames: Vec<Stackframe>,
}

impl CoredumpNotesHeader {
    fn new(page_size: u64) -> Self {
        Self {
            page_size,
            frames: Vec::new(),
        }
    }
    fn add_frame(&mut self, frame: Stackframe) {
        self.frames.push(frame);
    }
}
impl Display for CoredumpNotesHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Size: {}", self.page_size)?;
        for frame in &self.frames {
            writeln!(f, "{}", frame)?;
        }
        Ok(())
    }
}

fn parse_backtrace_notes<Elf>(
    endian: Elf::Endian,
    bt: &object::read::elf::Note<Elf>,
) -> Result<CoredumpNotesHeader, Box<dyn Error>>
where
    Elf: FileHeader<Endian = Endianness>,
{
    let mut cursor = Cursor::new(bt.desc());
    let number_of_entries = cursor.read_u64::<NativeEndian>()?;
    let page_size = cursor.read_u64::<NativeEndian>()?;

    let mut name_cursor = Cursor::new(bt.desc());
    name_cursor.set_position(cursor.position() + number_of_entries * 24);

    let mut headers = CoredumpNotesHeader::new(page_size);
    for name in name_cursor.split(0).flatten() {
        let start = cursor.read_u64::<NativeEndian>()?;
        let end = cursor.read_u64::<NativeEndian>()?;
        let offset = cursor.read_u64::<NativeEndian>()?;
        let name = String::from_utf8_lossy(&name).into_owned();

        headers.add_frame(Stackframe {
            start,
            end,
            offset,
            name,
        });
    }

    Ok(headers)
}

fn read_frames<Elf: FileHeader<Endian = Endianness>>(
    object: &[u8],
) -> Result<CoredumpNotesHeader, Box<dyn Error>> {
    let elf = Elf::parse(object)?;
    let endian = elf.endian()?;
    // verify this is a core file
    if elf.e_type(endian) != elf::ET_CORE {
        Err(CoredumpError::NotCoreFile)?;
    }

    for header in elf.program_headers(endian, object)? {
        if header.p_type(endian) == elf::PT_NOTE {
            if let Ok(Some(mut notes)) = header.notes(endian, object) {
                while let Ok(Some(note)) = notes.next() {
                    if note.n_type(endian) == elf::NT_FILE {
                        let notes = parse_backtrace_notes(endian, &note)?;
                        return Ok(notes);
                    }
                }
            }
        }
    }
    Err(CoredumpError::MissingDataSection)?
}

fn symbolicate_notes(notes: CoredumpNotesHeader) -> Result<Vec<Frame>, Box<dyn Error>> {
    todo!("Symbolize me!");
    Err(CoredumpError::SymbolizationFailed)?
}

fn parse_coredump<P: AsRef<Path>>(path: P) -> Result<Stacktrace, Box<dyn Error>> {
    let bin_data = fs::read(path)?;
    let file_kind = object::FileKind::parse(&*bin_data)?;
    let notes = match file_kind {
        object::FileKind::Elf64 => read_frames::<elf::FileHeader64<Endianness>>(&bin_data),
        object::FileKind::Elf32 => read_frames::<elf::FileHeader32<Endianness>>(&bin_data),
        _ => Err(CoredumpError::FileFormatNotSupported)?,
    }?;
    println!("Headers: {}", notes);

    let frames = symbolicate_notes(notes)?;

    Ok(Stacktrace {
        registers: BTreeMap::new(),
        frames: Vec::new(),
        frames_omitted: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works() {
        let coredump = parse_coredump("/home/lpraneis/personal/src/sample/core");
        println!("{:?}", coredump);
    }
}
