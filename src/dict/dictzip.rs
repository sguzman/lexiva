//! Bounded, random-access dictzip (GNU DICT .dict.dz) version 1 decoder.
//!
//! RFC 1952 gzip FEXTRA contains an "RA" subfield with version, plain
//! chunk length, chunk count and two-byte compressed lengths. Chunks are
//! raw DEFLATE streams ending in Z_FULL_FLUSH markers, and are independently
//! decompressible. Original dictionary files are never modified.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

use flate2::{Decompress, FlushDecompress};

const MAX_GZIP_TEXT_FIELD: usize = 4096;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

#[derive(Debug)]
pub(super) struct DictzipReader {
    file: File,
    chunk_length: u64,
    length: u64,
    chunks: Vec<(u64, usize)>,
    cached: Option<(usize, Vec<u8>)>,
}

impl DictzipReader {
    pub(super) fn open(mut file: File) -> io::Result<Self> {
        let file_length = file.metadata()?.len();
        if file_length < 22 {
            return Err(invalid("truncated dictzip gzip header"));
        }
        let mut header = [0u8; 10];
        file.read_exact(&mut header)?;
        if header[..3] != [0x1f, 0x8b, 8] {
            return Err(invalid("dictzip requires a gzip DEFLATE stream"));
        }
        let flags = header[3];
        if flags & 0xe0 != 0 || flags & 0x04 == 0 {
            return Err(invalid("dictzip requires a supported gzip FEXTRA header"));
        }

        let mut extra_length = [0u8; 2];
        file.read_exact(&mut extra_length)?;
        let xlen = usize::from(read_u16(&extra_length));
        let mut extra = vec![0u8; xlen];
        file.read_exact(&mut extra)?;

        let mut table: Option<(u64, Vec<usize>)> = None;
        let mut pos = 0usize;
        while pos < extra.len() {
            if extra.len() - pos < 4 {
                return Err(invalid("truncated gzip extra subfield"));
            }
            let field_id = &extra[pos..pos + 2];
            let len = usize::from(read_u16(&extra[pos + 2..pos + 4]));
            pos += 4;
            if len > extra.len() - pos {
                return Err(invalid("gzip extra field overruns XLEN"));
            }
            if field_id == b"RA" {
                if table.is_some() || len < 6 {
                    return Err(invalid("duplicate or truncated dictzip RA field"));
                }
                let field = &extra[pos..pos + len];
                let version = read_u16(&field[0..2]);
                let chunk_length = u64::from(read_u16(&field[2..4]));
                let count = usize::from(read_u16(&field[4..6]));
                if version != 1 || chunk_length == 0 || count == 0 {
                    return Err(invalid("unsupported dictzip RA version or chunk layout"));
                }
                if len != 6 + count * 2 {
                    return Err(invalid("dictzip RA chunk table length mismatch"));
                }
                let mut lengths = Vec::with_capacity(count);
                for i in 0..count {
                    let compressed = usize::from(read_u16(&field[6 + 2 * i..8 + 2 * i]));
                    if compressed == 0 {
                        return Err(invalid("zero-length dictzip chunk"));
                    }
                    lengths.push(compressed);
                }
                table = Some((chunk_length, lengths));
            }
            pos += len;
        }

        // Optional gzip text fields have an explicit small upper bound.
        if flags & 0x08 != 0 {
            skip_nul_terminated(&mut file)?;
        }
        if flags & 0x10 != 0 {
            skip_nul_terminated(&mut file)?;
        }
        if flags & 0x02 != 0 {
            let mut header_crc = [0u8; 2];
            file.read_exact(&mut header_crc)?;
        }
        let payload_start = file.stream_position()?;
        let (chunk_length, compressed_lengths) =
            table.ok_or_else(|| invalid("gzip file lacks dictzip RA chunk table"))?;
        let mut trailer = [0u8; 8];
        file.seek(SeekFrom::End(-8))?;
        file.read_exact(&mut trailer)?;
        let length = u64::from(u32::from_le_bytes([
            trailer[4], trailer[5], trailer[6], trailer[7],
        ]));
        // Dictzip v1's 16-bit chunk table limits the format to under 4 GiB.
        let chunks_required = length
            .checked_add(chunk_length - 1)
            .ok_or_else(|| invalid("dictzip uncompressed length overflow"))?
            / chunk_length;
        if length == 0 || chunks_required != compressed_lengths.len() as u64 {
            return Err(invalid("dictzip RA chunk count and gzip ISIZE disagree"));
        }

        let payload_end = file_length - 8;
        let mut offset = payload_start;
        let mut chunks = Vec::with_capacity(compressed_lengths.len());
        for compressed in compressed_lengths {
            let end = offset
                .checked_add(compressed as u64)
                .ok_or_else(|| invalid("dictzip compressed offsets overflow"))?;
            if end > payload_end {
                return Err(invalid("dictzip chunk overlaps gzip trailer"));
            }
            chunks.push((offset, compressed));
            offset = end;
        }
        // The gzip member's final DEFLATE end block follows the RA chunks.
        if offset >= payload_end {
            return Err(invalid("dictzip missing final DEFLATE end block"));
        }
        Ok(Self {
            file,
            chunk_length,
            length,
            chunks,
            cached: None,
        })
    }

    pub(super) fn len(&self) -> u64 {
        self.length
    }

    fn read_chunk(&mut self, index: usize) -> io::Result<&[u8]> {
        if self.cached.as_ref().is_some_and(|(cached, _)| *cached == index) {
            return Ok(&self.cached.as_ref().expect("cache checked above").1);
        }
        let (offset, compressed_len) = self.chunks[index];
        self.file.seek(SeekFrom::Start(offset))?;
        let mut compressed = vec![0u8; compressed_len];
        self.file.read_exact(&mut compressed)?;
        let expected = if index + 1 == self.chunks.len() {
            (self.length - self.chunk_length * index as u64) as usize
        } else {
            self.chunk_length as usize
        };
        // One spare byte detects a chunk that expands beyond advertised size.
        let mut output = vec![0u8; expected + 1];
        let mut decoder = Decompress::new(false);
        let mut consumed = 0usize;
        let mut written = 0usize;
        while consumed < compressed.len() {
            let old_in = decoder.total_in();
            let old_out = decoder.total_out();
            decoder
                .decompress(
                    &compressed[consumed..],
                    &mut output[written..],
                    FlushDecompress::Sync,
                )
                .map_err(|_| invalid("invalid dictzip raw DEFLATE chunk"))?;
            consumed = decoder.total_in() as usize;
            written = decoder.total_out() as usize;
            if written > expected {
                return Err(invalid("dictzip chunk exceeds declared uncompressed length"));
            }
            if decoder.total_in() == old_in && decoder.total_out() == old_out {
                return Err(invalid("dictzip decoder stopped before consuming chunk"));
            }
        }
        if written != expected {
            return Err(invalid("dictzip chunk length differs from RA/ISIZE metadata"));
        }
        output.truncate(written);
        self.cached = Some((index, output));
        Ok(&self.cached.as_ref().expect("cache assigned above").1)
    }

    pub(super) fn read_at(&mut self, start: u64, length: u64) -> io::Result<Vec<u8>> {
        let end = start
            .checked_add(length)
            .ok_or_else(|| invalid("dictzip read position overflow"))?;
        if end > self.length || length > super::MAX_ARTICLE_BYTES {
            return Err(invalid("dictzip read outside uncompressed data bounds"));
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let mut result = Vec::with_capacity(length as usize);
        let mut position = start;
        while position < end {
            let chunk_index = (position / self.chunk_length) as usize;
            let within = (position % self.chunk_length) as usize;
            let chunk = self.read_chunk(chunk_index)?;
            let take = (end - position).min((chunk.len() - within) as u64) as usize;
            if take == 0 {
                return Err(invalid("dictzip decoder made no forward progress"));
            }
            result.extend_from_slice(&chunk[within..within + take]);
            position += take as u64;
        }
        Ok(result)
    }
}

fn skip_nul_terminated(file: &mut File) -> io::Result<()> {
    for _ in 0..MAX_GZIP_TEXT_FIELD {
        let mut byte = [0u8; 1];
        file.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return Ok(());
        }
    }
    Err(invalid("gzip text field exceeds limit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const RAW: &[u8] = b"house; home\nblue sky\nneedle\n\xc3\xa1rbol: tree\n";

    fn fixture() -> (File, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "lexiva-dictzip-{}-{}.dz",
            std::process::id(),
            std::thread::current().name().unwrap_or("thread")
        ));
        fs::write(&path, include_bytes!("../../tests/fixtures/synthetic.dict.dz")).unwrap();
        (File::open(&path).unwrap(), path)
    }

    #[test]
    fn spans_many_chunks_and_handles_exact_boundary() {
        let (file, path) = fixture();
        let mut reader = DictzipReader::open(file).unwrap();
        assert_eq!(reader.len(), RAW.len() as u64);
        assert_eq!(reader.read_at(0, 13).unwrap(), &RAW[..13]);
        assert_eq!(reader.read_at(12, 17).unwrap(), &RAW[12..29]);
        assert_eq!(reader.read_at(28, 13).unwrap(), &RAW[28..]);
        assert_eq!(reader.read_at(13, 13).unwrap(), &RAW[13..26]);
        assert!(reader.read_at(RAW.len() as u64, 1).is_err());
        assert!(reader.read_at(u64::MAX, 2).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reject_corrupt_ra_header_and_chunk_offsets() {
        let (_, path) = fixture();
        let original = fs::read(&path).unwrap();
        // Unknown RA format version, a bad chunk count, and compressed
        // table sizes that overlap the gzip trailer must not be accepted.
        let position = original.windows(2).position(|w| w == b"RA").unwrap();
        let mut changed_version = original.clone();
        changed_version[position + 4] = 2;
        let mut changed_count = original.clone();
        changed_count[position + 8] = 5;
        let mut changed_chunk_length = original.clone();
        changed_chunk_length[position + 6] = 0;
        changed_chunk_length[position + 7] = 0;
        let mut changed_size = original.clone();
        changed_size[position + 10] = 255;
        changed_size[position + 11] = 255;
        for bytes in [
            changed_version,
            changed_count,
            changed_chunk_length,
            changed_size,
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(DictzipReader::open(File::open(&path).unwrap()).is_err());
        }
        fs::write(&path, original).unwrap();
        fs::remove_file(path).unwrap();
    }
}
