//! Read-only GNU DICT .index reader for plain and dictzip data.
//!
//! A .index line is: headword TAB base64(offset) TAB base64(length) LF.
//! Offsets and lengths refer to bytes in the *uncompressed* .dict data.
mod dictzip;
pub(crate) mod metadata;

#[cfg(test)]
mod real_freedict_tests;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};

use std::path::PathBuf;

use serde_json::json;

use crate::model::{DictionarySource, LexicalEntry, Sense};
use crate::storage::{Result as StoreResult, StoreError};

const MAX_INDEX_LINE_BYTES: u64 = 16 * 1024;
const MAX_ARTICLE_BYTES: u64 = 4 * 1024 * 1024;
const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Files are discovered, not changed. A compressed pair may be listed but
/// requires the dictzip adapter before its entries can be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DictFiles {
    pub index: PathBuf,
    pub data: PathBuf,
    pub compressed: bool,
}

impl DictFiles {
    fn from_index(index: PathBuf) -> Option<Self> {
        if index.extension().and_then(|s| s.to_str()) != Some("index") {
            return None;
        }
        let plain = index.with_extension("dict");
        let compressed = index.with_extension("dict.dz");
        if plain.is_file() {
            Some(Self {
                index,
                data: plain,
                compressed: false,
            })
        } else if compressed.is_file() {
            Some(Self {
                index,
                data: compressed,
                compressed: true,
            })
        } else {
            None
        }
    }
}

/// Common Arch paths; no server or privileged operation is required.
pub fn system_dictd_directories() -> [PathBuf; 2] {
    [
        PathBuf::from("/usr/share/dictd"),
        PathBuf::from("/usr/local/share/dictd"),
    ]
}

/// Non-recursive discovery, including explicit user-selected directories.
/// Absent directories are ignored, but unreadable directories report errors.
/// Sources are ordered by their index path and are never opened for writing.
pub fn discover_in(directories: &[PathBuf]) -> io::Result<Vec<DictFiles>> {
    let mut by_index = BTreeMap::new();
    for directory in directories {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for item in entries {
            let path = item?.path();
            if let Some(files) = DictFiles::from_index(path) {
                by_index.insert(files.index.clone(), files);
            }
        }
    }
    Ok(by_index.into_values().collect())
}

pub fn discover_system() -> io::Result<Vec<DictFiles>> {
    discover_in(&system_dictd_directories())
}

fn invalid(reason: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.into())
}

fn decode_position(input: &str) -> io::Result<u64> {
    if input.is_empty() {
        return Err(invalid("empty DICT base64 offset or length"));
    }
    let mut value = 0_u64;
    for digit in input.bytes() {
        let place = DIGITS
            .iter()
            .position(|candidate| *candidate == digit)
            .ok_or_else(|| invalid("invalid DICT base64 digit"))? as u64;
        value = value
            .checked_mul(64)
            .and_then(|number| number.checked_add(place))
            .ok_or_else(|| invalid("DICT base64 integer overflow"))?;
    }
    Ok(value)
}

struct IndexRow<'a> {
    lemma: &'a str,
    start: u64,
    length: u64,
}

fn parse_row(line: &str) -> io::Result<IndexRow<'_>> {
    let mut parts = line.split('\t');
    let lemma = parts.next().ok_or_else(|| invalid("missing headword"))?;
    let start = parts.next().ok_or_else(|| invalid("missing offset"))?;
    let length = parts.next().ok_or_else(|| invalid("missing length"))?;
    if parts.next().is_some() {
        return Err(invalid("unexpected extra DICT index field"));
    }
    // Some distributed dictd indexes contain a well-formed record whose
    // headword is empty (often the first row). Accept its structure here;
    // readers skip it after independently validating its byte range.
    Ok(IndexRow {
        lemma,
        start: decode_position(start)?,
        length: decode_position(length)?,
    })
}

enum DictBody {
    Plain { file: File, len: u64 },
    Zipped(dictzip::DictzipReader),
}

impl DictBody {
    fn len(&self) -> u64 {
        match self {
            Self::Plain { len, .. } => *len,
            Self::Zipped(reader) => reader.len(),
        }
    }

    fn read_at(&mut self, start: u64, length: u64) -> io::Result<Vec<u8>> {
        match self {
            Self::Plain { file, .. } => {
                file.seek(SeekFrom::Start(start))?;
                let mut result = vec![0u8; length as usize];
                file.read_exact(&mut result)?;
                Ok(result)
            }
            Self::Zipped(reader) => reader.read_at(start, length),
        }
    }
}

pub struct DictReader {
    source_id: String,
    headword_language: Option<String>,
    definition_language: Option<String>,
    index: BufReader<File>,
    data: DictBody,
    line_no: u64,
    finished: bool,
}

impl DictReader {
    pub fn open(source: &DictionarySource, files: &DictFiles) -> io::Result<Self> {
        source
            .validate()
            .map_err(|reason| invalid(format!("invalid dictionary source: {reason}")))?;
        let file = File::open(&files.data)?;
        let data = if files.compressed {
            DictBody::Zipped(dictzip::DictzipReader::open(file)?)
        } else {
            let len = file.metadata()?.len();
            DictBody::Plain { file, len }
        };
        Ok(Self {
            source_id: source.id.clone(),
            headword_language: source.headword_language.clone(),
            definition_language: source.definition_language.clone(),
            index: BufReader::new(File::open(&files.index)?),
            data,
            line_no: 0,
            finished: false,
        })
    }

    fn read_entry(&mut self) -> io::Result<Option<LexicalEntry>> {
        loop {
            let mut line = Vec::new();
            let read = self
                .index
                .by_ref()
                .take(MAX_INDEX_LINE_BYTES + 1)
                .read_until(b'\n', &mut line)?;
            if read == 0 {
                return Ok(None);
            }
            if line.len() as u64 > MAX_INDEX_LINE_BYTES {
                return Err(invalid("DICT index line exceeds size limit"));
            }
            self.line_no += 1;
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let text = std::str::from_utf8(&line)
                .map_err(|_| invalid("DICT index headword is not UTF-8"))?;
            let row = parse_row(text)
                .map_err(|error| invalid(format!("DICT index line {}: {error}", self.line_no)))?;
            if row.length > MAX_ARTICLE_BYTES {
                return Err(invalid("DICT article exceeds size limit"));
            }
            if row
                .start
                .checked_add(row.length)
                .is_none_or(|end| end > self.data.len())
            {
                return Err(invalid(format!(
                    "DICT index line {} points past data file",
                    self.line_no
                )));
            }
            // Empty-headword records occur in distributed FreeDict indexes.
            // They are not searchable entries, but their offsets and lengths
            // have already passed the same strict bounds checks as words.
            if row.lemma.is_empty() {
                continue;
            }
            // Dictd administrative records are source metadata, not headwords.
            if row.lemma.starts_with("00-database-") || row.lemma == "00databaseshort" {
                continue;
            }
            let bytes = self.data.read_at(row.start, row.length)?;
            let article =
                String::from_utf8(bytes).map_err(|_| invalid("DICT article is not UTF-8"))?;
            let part_of_speech = metadata::article_part_of_speech(row.lemma, &article);
            return Ok(Some(LexicalEntry {
                source_id: self.source_id.clone(),
                source_entry_id: format!("index-line:{}", self.line_no),
                lemma: row.lemma.to_owned(),
                headword_language: self.headword_language.clone(),
                definition_language: self.definition_language.clone(),
                part_of_speech,
                senses: vec![Sense {
                    number: None,
                    definition_language: self.definition_language.clone(),
                    glosses: vec![article],
                    labels: vec![],
                    examples: vec![],
                    children: vec![],
                }],
                pronunciations: vec![],
                etymologies: vec![],
                forms: vec![],
                relations: vec![],
                metadata: json!({
                    "dict_index_line": self.line_no,
                    "dict_data_offset": row.start,
                    "dict_article_bytes": row.length,
                    "dict_article_format": "plain_text_verbatim",
                }),
            }));
        }
    }
}

impl Iterator for DictReader {
    type Item = StoreResult<LexicalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.read_entry() {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(StoreError::Io(error)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SourceFormat;
    use crate::storage::DictionaryStore;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct FixtureDir(PathBuf);

    impl FixtureDir {
        fn new() -> Self {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("lexiva-dict-{}-{id}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn pair(&self, name: &str, index: &[u8], data: &[u8]) -> DictFiles {
            let index_path = self.0.join(format!("{name}.index"));
            let data_path = self.0.join(format!("{name}.dict"));
            fs::write(&index_path, index).unwrap();
            fs::write(&data_path, data).unwrap();
            DictFiles {
                index: index_path,
                data: data_path,
                compressed: false,
            }
        }
    }

    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn encode(mut value: u64) -> String {
        if value == 0 {
            return "A".to_owned();
        }
        let mut output = Vec::new();
        while value != 0 {
            output.push(DIGITS[(value % 64) as usize] as char);
            value /= 64;
        }
        output.into_iter().rev().collect()
    }

    fn source() -> DictionarySource {
        DictionarySource {
            id: "es-en".into(),
            title: "Synthetic Spanish–English".into(),
            format: SourceFormat::Dict,
            origin: None,
            headword_language: Some("es".into()),
            definition_language: Some("en".into()),
            license: None,
            attribution: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn base64_offsets_use_dict_alphabet_and_detect_overflow() {
        assert_eq!(decode_position("A").unwrap(), 0);
        assert_eq!(decode_position("B").unwrap(), 1);
        assert_eq!(decode_position("/").unwrap(), 63);
        assert_eq!(decode_position("BA").unwrap(), 64);
        assert_eq!(decode_position("B/").unwrap(), 127);
        assert_eq!(decode_position("+").unwrap(), 62);
        for invalid in ["", "A=", "-", "1_2", "ZZZZZZZZZZZZZZZZZZ"] {
            assert!(decode_position(invalid).is_err());
        }
        assert_eq!(decode_position(&encode(2_000_000)).unwrap(), 2_000_000);
    }

    #[test]
    fn discovery_finds_plain_and_compressed_pairs_in_selected_directories() {
        let fixture = FixtureDir::new();
        let plain = fixture.pair("es-en", b"casa\tA\tB\n", b"a");
        fs::write(fixture.0.join("missing.index"), b"").unwrap();
        fs::write(fixture.0.join("es-es.index"), b"").unwrap();
        fs::write(fixture.0.join("es-es.dict.dz"), b"not decoded yet").unwrap();
        let dirs = [fixture.0.clone(), fixture.0.clone()];
        let discovered = discover_in(&dirs).unwrap();
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0], plain);
        assert!(discovered[1].compressed);
        assert!(DictReader::open(&source(), &discovered[1]).is_err());
    }

    #[test]
    fn reads_unicode_lemmas_and_homographs_without_byte_offset_drift() {
        let fixture = FixtureDir::new();
        let data = "house; home\nblue room\nother meaning";
        let first = "house; home\n".len() as u64;
        let second = "blue room\n".len() as u64;
        let index = format!(
            "00-database-info\tA\tA\ncasa\tA\t{}\nárbol\t{}\t{}\ncasa\t{}\t{}\n",
            encode(first),
            encode(first),
            encode(second),
            encode(first + second),
            encode("other meaning".len() as u64)
        );
        let files = fixture.pair("es-en", index.as_bytes(), data.as_bytes());
        let words: Vec<_> = DictReader::open(&source(), &files)
            .unwrap()
            .collect::<StoreResult<Vec<_>>>()
            .unwrap();
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].lemma, "casa");
        assert_eq!(words[0].senses[0].glosses[0], "house; home\n");
        assert_eq!(words[1].lemma, "árbol");
        assert_eq!(words[1].senses[0].glosses[0], "blue room\n");
        assert_eq!(words[2].lemma, "casa");
        assert_eq!(words[2].senses[0].glosses[0], "other meaning");
        assert_ne!(words[0].source_entry_id, words[2].source_entry_id);
        assert_eq!(words[1].headword_language.as_deref(), Some("es"));
        assert_eq!(words[1].definition_language.as_deref(), Some("en"));
    }

    #[test]
    fn imports_plain_dict_into_source_scoped_search_index() {
        let fixture = FixtureDir::new();
        let data = "house\nhome\n";
        let index = format!(
            "casa\tA\t{}\nhogar\t{}\t{}\n",
            encode("house\n".len() as u64),
            encode("house\n".len() as u64),
            encode("home\n".len() as u64),
        );
        let files = fixture.pair("es-en", index.as_bytes(), data.as_bytes());
        let db_path = fixture.0.join("catalog.sqlite");
        let mut db = DictionaryStore::open(&db_path).unwrap();
        db.upsert_source(&source()).unwrap();
        let count = db
            .import_stream(
                "es-en",
                DictReader::open(&source(), &files).unwrap(),
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(db.count_entries("es-en").unwrap(), 2);
        let results = db.search_prefix("cas", Some("es-en"), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lemma, "casa");
        assert_eq!(results[0].senses[0].glosses[0], "house\n");
        db.check_fts_index().unwrap();
    }

    #[test]
    fn malformed_offsets_and_oversized_indexes_abort_without_live_changes() {
        let fixture = FixtureDir::new();
        let files = fixture.pair("bad", b"casa\tA\tZZZZZZ\n", b"house");
        let mut reader = DictReader::open(&source(), &files).unwrap();
        assert!(reader.next().unwrap().is_err());
        assert!(reader.next().is_none());

        let db_path = fixture.0.join("catalog.sqlite");
        let mut db = DictionaryStore::open(&db_path).unwrap();
        db.upsert_source(&source()).unwrap();
        let original = LexicalEntry {
            source_id: "es-en".into(),
            source_entry_id: "prior".into(),
            lemma: "old".into(),
            headword_language: Some("es".into()),
            definition_language: Some("en".into()),
            part_of_speech: None,
            senses: vec![],
            pronunciations: vec![],
            etymologies: vec![],
            forms: vec![],
            relations: vec![],
            metadata: Value::Null,
        };
        db.replace_source_entries("es-en", &[original]).unwrap();
        assert!(
            db.import_stream(
                "es-en",
                DictReader::open(&source(), &files).unwrap(),
                || false,
                |_| {},
            )
            .is_err()
        );
        assert_eq!(db.count_entries("es-en").unwrap(), 1);
        assert_eq!(db.search_prefix("old", Some("es-en"), 10).unwrap().len(), 1);

        let huge = vec![b'x'; MAX_INDEX_LINE_BYTES as usize + 1];
        let huge_files = fixture.pair("huge", &huge, b"x");
        let mut reader = DictReader::open(&source(), &huge_files).unwrap();
        assert!(reader.next().unwrap().is_err());
    }

    #[test]
    fn compressed_dictzip_import_uses_uncompressed_index_positions() {
        let fixture = FixtureDir::new();
        let raw = b"house; home\nblue sky\nneedle\n\xc3\xa1rbol: tree\n";
        let sample = include_bytes!("../tests/fixtures/synthetic.dict.dz");
        let index = format!(
            "casa\tA\t{}\nazul\t{}\t{}\npunta\t{}\t{}\narbol\t{}\t{}\n",
            encode(12),
            encode(12),
            encode(9),
            encode(21),
            encode(7),
            encode(28),
            encode(13)
        );
        let index_path = fixture.0.join("compressed.index");
        let data_path = fixture.0.join("compressed.dict.dz");
        fs::write(&index_path, index).unwrap();
        fs::write(&data_path, sample).unwrap();
        let files = DictFiles {
            index: index_path,
            data: data_path,
            compressed: true,
        };
        let mut reader = DictReader::open(&source(), &files).unwrap();
        let first = reader.next().unwrap().unwrap();
        let second = reader.next().unwrap().unwrap();
        assert_eq!(first.senses[0].glosses[0], "house; home\n");
        assert_eq!(second.senses[0].glosses[0], "blue sky\n");
        let rest: Vec<_> = reader.collect::<StoreResult<Vec<_>>>().unwrap();
        assert_eq!(rest[1].senses[0].glosses[0], "árbol: tree\n");
        assert_eq!(raw.len(), 41);

        let mut store = DictionaryStore::open(&fixture.0.join("dictzip.sqlite")).unwrap();
        store.upsert_source(&source()).unwrap();
        let imported = store
            .import_stream(
                "es-en",
                DictReader::open(
                    &source(),
                    &DictFiles {
                        index: files.index.clone(),
                        data: files.data.clone(),
                        compressed: true,
                    },
                )
                .unwrap(),
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(imported, 4);
        assert_eq!(
            store.search_prefix("cas", Some("es-en"), 10).unwrap().len(),
            1
        );
        store.check_fts_index().unwrap();
    }

    #[test]
    fn leading_empty_headword_is_skipped_but_following_words_import() {
        let fixture = FixtureDir::new();
        let header = "English - Spanish FreeDict";
        let article = "house - casa\n";
        let data = format!("{header}{article}");
        let index = format!(
            "\tA\t{}\n00-database-short\tA\t{}\nhouse\t{}\t{}\n",
            encode(header.len() as u64),
            encode(header.len() as u64),
            encode(header.len() as u64),
            encode(article.len() as u64),
        );
        let files = fixture.pair("eng-spa", index.as_bytes(), data.as_bytes());
        let source = metadata::source_from_files(&files, "en-es").unwrap();
        assert_eq!(source.title, header);
        assert_eq!(source.headword_language.as_deref(), Some("en"));
        assert_eq!(source.definition_language.as_deref(), Some("es"));

        let entries: Vec<_> = DictReader::open(&source, &files)
            .unwrap()
            .collect::<StoreResult<Vec<_>>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].lemma, "house");
        assert_eq!(entries[0].source_entry_id, "index-line:3");
        assert_eq!(entries[0].senses[0].glosses[0], article);

        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source).unwrap();
        assert_eq!(
            store
                .import_stream(
                    &source.id,
                    DictReader::open(&source, &files).unwrap(),
                    || false,
                    |_| {},
                )
                .unwrap(),
            1
        );
        assert_eq!(store.search_prefix("hou", Some(&source.id), 10).unwrap().len(), 1);
        store.check_fts_index().unwrap();
    }

    #[test]
    fn truncated_and_invalid_index_rows_fail_closed() {
        for row in [
            "casa\tA\n",
            "casa\tA\t\n",
            "casa\tA\tB\textra\n",
            "\tA\n",
            "\tA\t\n",
            "\tA\tB\textra\n",
            "\t*\tB\n",
            "\tB\tB\n",
            "casa\t*\tB\n",
        ] {
            let fixture = FixtureDir::new();
            let files = fixture.pair("invalid", row.as_bytes(), b"x");
            let mut reader = DictReader::open(&source(), &files).unwrap();
            assert!(reader.next().unwrap().is_err(), "row: {row:?}");
        }
    }
}
