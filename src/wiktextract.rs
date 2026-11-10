//! Streaming Wiktextract JSONL and JSONL.gz adapter.
//!
//! lang_code identifies the headword language; the Wiktionary edition
//! determines the language in which definitions were written. This is never
//! inferred from a word or filename.
//!
//! Original files remain read-only. An entry retains its complete structured
//! record for provenance and future format evolution.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use flate2::read::MultiGzDecoder;
use serde_json::{Value, json};

use crate::model::{
    DictionarySource, Example, LexicalEntry, Pronunciation, RelatedTerm, Sense, SourceFormat,
    WordForm, valid_language_tag,
};
use crate::storage::{Result as StoreResult, StoreError};

const MAX_JSONL_LINE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WiktionaryEdition {
    English,
    Spanish,
}

impl WiktionaryEdition {
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Explicitly declares definition language by Wiktionary edition.
/// Dictionary licensing cannot be established from a filename.
pub fn source_from_jsonl(
    path: &Path,
    id: &str,
    title: &str,
    edition: WiktionaryEdition,
    word_language: Option<&str>,
) -> io::Result<DictionarySource> {
    let source = DictionarySource {
        id: id.to_owned(),
        title: title.to_owned(),
        format: SourceFormat::Wiktextract,
        origin: Some(path.to_string_lossy().into_owned()),
        headword_language: word_language.map(str::to_owned),
        definition_language: Some(edition.code().to_owned()),
        license: None,
        attribution: Some("Wiktionary contributors; extracted with Wiktextract".to_owned()),
        metadata: json!({
            "wiktionary_edition": edition.code(),
            "source_file": path.to_string_lossy(),
            "license_must_be_reviewed": true
        }),
    };
    source.validate().map_err(invalid)?;
    Ok(source)
}

pub struct WiktextractReader {
    reader: BufReader<Box<dyn Read + Send>>,
    line: Vec<u8>,
    source_id: String,
    word_language_filter: Option<String>,
    edition: WiktionaryEdition,
    line_number: u64,
    finished: bool,
}

impl WiktextractReader {
    pub fn open(
        path: &Path,
        source: &DictionarySource,
        edition: WiktionaryEdition,
    ) -> io::Result<Self> {
        source.validate().map_err(invalid)?;
        if source.format != SourceFormat::Wiktextract {
            return Err(invalid("Wiktextract reader requires a Wiktextract source"));
        }
        if source.definition_language.as_deref() != Some(edition.code()) {
            return Err(invalid(
                "Wiktionary edition does not match source definition language",
            ));
        }
        let name = path
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or("");
        let file = File::open(path)?;
        let input: Box<dyn Read + Send> = if name.ends_with(".jsonl.gz") {
            Box::new(MultiGzDecoder::new(file))
        } else if name.ends_with(".jsonl") {
            Box::new(file)
        } else {
            return Err(invalid("expected a .jsonl or .jsonl.gz file"));
        };
        Ok(Self {
            reader: BufReader::new(input),
            line: Vec::new(),
            source_id: source.id.clone(),
            word_language_filter: source.headword_language.clone(),
            edition,
            line_number: 0,
            finished: false,
        })
    }

    fn read_entry(&mut self) -> StoreResult<Option<LexicalEntry>> {
        loop {
            self.line.clear();
            let read = (&mut self.reader)
                .take(MAX_JSONL_LINE_BYTES + 1)
                .read_until(b'\n', &mut self.line)
                .map_err(StoreError::Io)?;
            if read == 0 {
                return Ok(None);
            }
            self.line_number += 1;
            if self.line.len() as u64 > MAX_JSONL_LINE_BYTES {
                return Err(StoreError::Io(invalid(format!(
                    "Wiktextract line {} exceeds 2 MiB input limit",
                    self.line_number
                ))));
            }
            if self.line.last() == Some(&b'\n') {
                self.line.pop();
            }
            if self.line.last() == Some(&b'\r') {
                self.line.pop();
            }
            if self.line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let record: Value = serde_json::from_slice(&self.line).map_err(|error| {
                StoreError::Io(invalid(format!(
                    "invalid Wiktextract JSON on line {}: {error}",
                    self.line_number
                )))
            })?;
            let lang = field(&record, "lang_code").ok_or_else(|| {
                StoreError::Io(invalid(format!(
                    "Wiktextract line {} has no lang_code",
                    self.line_number
                )))
            })?;
            // Do not index every language when a single language was selected.
            if self
                .word_language_filter
                .as_deref()
                .is_some_and(|filter| filter != lang)
            {
                continue;
            }
            return Ok(Some(map_record(
                record,
                &self.source_id,
                self.edition,
                self.line_number,
            )?));
        }
    }
}

impl Iterator for WiktextractReader {
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
                Some(Err(error))
            }
        }
    }
}

fn field<'a>(record: &'a Value, key: &str) -> Option<&'a str> {
    record.get(key).and_then(Value::as_str)
}

fn strings(record: &Value, key: &str) -> Vec<String> {
    record
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn map_record(
    raw: Value,
    source_id: &str,
    edition: WiktionaryEdition,
    line_number: u64,
) -> StoreResult<LexicalEntry> {
    let lemma = field(&raw, "word")
        .filter(|word| !word.is_empty())
        .ok_or(StoreError::Invalid("Wiktextract word is missing"))?
        .to_owned();
    let language = field(&raw, "lang_code")
        .filter(|tag| valid_language_tag(tag))
        .ok_or(StoreError::Invalid("invalid Wiktextract lang_code"))?
        .to_owned();
    let mut senses = Vec::new();
    if let Some(raw_senses) = raw.get("senses").and_then(Value::as_array) {
        for (position, value) in raw_senses.iter().enumerate() {
            let examples = value
                .get("examples")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    Some(Example {
                        text: field(item, "text")?.to_owned(),
                        translation: field(item, "translation")
                            .or_else(|| field(item, "english"))
                            .map(str::to_owned),
                        reference: field(item, "ref").map(str::to_owned),
                    })
                })
                .collect();
            let mut labels = strings(value, "tags");
            labels.extend(strings(value, "raw_tags"));
            labels.extend(strings(value, "topics"));
            let mut glosses = strings(value, "glosses");
            if glosses.is_empty() {
                glosses = strings(value, "raw_glosses");
            }
            senses.push(Sense {
                number: Some((position + 1).to_string()),
                definition_language: Some(edition.code().to_owned()),
                glosses,
                labels,
                examples,
                children: Vec::new(),
            });
        }
    }

    let pronunciations = raw
        .get("sounds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|sound| {
            let ipa = field(sound, "ipa").map(str::to_owned);
            let audio = field(sound, "ogg_url")
                .or_else(|| field(sound, "audio"))
                .map(str::to_owned);
            if ipa.is_none() && audio.is_none() {
                return None;
            }
            Some(Pronunciation {
                ipa,
                audio,
                labels: strings(sound, "tags"),
            })
        })
        .collect();

    let forms = raw
        .get("forms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|form| {
            Some(WordForm {
                spelling: field(form, "form")?.to_owned(),
                tags: strings(form, "tags"),
            })
        })
        .collect();

    let mut relations = Vec::new();
    for (key, kind) in [
        ("synonyms", "synonym"),
        ("antonyms", "antonym"),
        ("related", "related"),
        ("derived", "derived"),
        ("hypernyms", "hypernym"),
        ("hyponyms", "hyponym"),
    ] {
        if let Some(items) = raw.get(key).and_then(Value::as_array) {
            for item in items {
                if let Some(word) = field(item, "word") {
                    relations.push(RelatedTerm {
                        kind: kind.to_owned(),
                        term: word.to_owned(),
                        language: Some(language.clone()),
                    });
                }
            }
        }
    }
    let etymologies = field(&raw, "etymology_text")
        .map(|text| vec![text.to_owned()])
        .unwrap_or_else(|| strings(&raw, "etymology_texts"));
    let part_of_speech = field(&raw, "pos").map(str::to_owned);
    let entry = LexicalEntry {
        source_id: source_id.to_owned(),
        // Different source lines may share a lemma and grammatical category.
        source_entry_id: format!("jsonl-line:{line_number}"),
        lemma,
        headword_language: Some(language),
        definition_language: Some(edition.code().to_owned()),
        part_of_speech,
        senses,
        pronunciations,
        etymologies,
        forms,
        relations,
        metadata: json!({
            "wiktionary_edition": edition.code(),
            "jsonl_line": line_number,
            "wiktextract_record": raw,
        }),
    };
    entry.validate().map_err(StoreError::Invalid)?;
    Ok(entry)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod large_tests;

#[cfg(test)]
mod real_tests;
