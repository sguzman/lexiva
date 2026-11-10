//! Opt-in tests against authentic, unbundled Kaikki.org raw per-word JSONL.
//! CI fetches current small records for both Wiktionary editions. The test
//! checks independent JSON parsing and the SQLite import path.

use super::*;

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::storage::DictionaryStore;

fn authentic_path(env_key: &str) -> PathBuf {
    PathBuf::from(env::var(env_key).expect("CI must provide authentic Wiktextract sample path"))
}

fn check_sample(env_key: &str, edition: WiktionaryEdition, word: &str) {
    let path = authentic_path(env_key);
    let contents = fs::read_to_string(&path).unwrap();
    let independent: Vec<Value> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect();
    assert!(!independent.is_empty(), "downloaded raw data was empty");
    assert!(independent.iter().any(|item| item["lang_code"] == "es"));

    let source = source_from_jsonl(
        &path,
        &format!("authentic-{}", edition.code()),
        "Kaikki authentic raw Wiktextract test",
        edition,
        Some("es"),
    )
    .unwrap();
    let entries = WiktextractReader::open(&path, &source, edition)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    let expected: Vec<&Value> = independent
        .iter()
        .filter(|record| record["lang_code"] == "es")
        .collect();
    assert_eq!(entries.len(), expected.len());
    assert!(entries.iter().all(|entry| entry.lemma == word));
    assert!(
        entries
            .iter()
            .all(|entry| entry.headword_language.as_deref() == Some("es"))
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry.definition_language.as_deref() == Some(edition.code()))
    );
    assert!(entries.iter().any(|entry| !entry.senses.is_empty()));
    for (entry, original) in entries.iter().zip(expected.iter()) {
        assert_eq!(&entry.metadata["wiktextract_record"], *original);
        assert_eq!(entry.part_of_speech.as_deref(), original["pos"].as_str());
        assert!(
            entry
                .senses
                .iter()
                .all(|sense| { sense.definition_language.as_deref() == Some(edition.code()) })
        );
        if let Some(glosses) = original["senses"].as_array() {
            assert_eq!(entry.senses.len(), glosses.len());
            for (sense, raw_sense) in entry.senses.iter().zip(glosses.iter()) {
                if let Some(expected_glosses) = raw_sense["glosses"].as_array() {
                    let formatted: Vec<&str> =
                        expected_glosses.iter().filter_map(Value::as_str).collect();
                    assert_eq!(
                        sense.glosses.iter().map(String::as_str).collect::<Vec<_>>(),
                        formatted
                    );
                }
            }
        }
    }
    let mut store = DictionaryStore::in_memory().unwrap();
    store.upsert_source(&source).unwrap();
    let count = store
        .import_stream(
            &source.id,
            WiktextractReader::open(&path, &source, edition).unwrap(),
            || false,
            |_| {},
        )
        .unwrap();
    assert_eq!(count, entries.len());
    assert_eq!(store.count_entries(&source.id).unwrap(), count as i64);
    let found = store.search_prefix("cas", Some(&source.id), 100).unwrap();
    assert!(!found.is_empty());
    assert!(found.iter().all(|entry| entry.lemma == word));
    store.check_fts_index().unwrap();

    eprintln!(
        "LEXIVA REAL WIKTEXTRACT: edition={} word={} records={} bytes={} pos={:?}",
        edition.code(),
        word,
        count,
        contents.len(),
        entries
            .iter()
            .map(|entry| entry.part_of_speech.as_deref())
            .collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "requires authentic per-word enwiktionary JSONL fetched in CI"]
fn authentic_english_edition_spanish_entry() {
    check_sample("LEXIVA_EN_REAL_JSONL", WiktionaryEdition::English, "casás");
}

#[test]
#[ignore = "requires authentic per-word eswiktionary JSONL fetched in CI"]
fn authentic_spanish_edition_spanish_entry() {
    check_sample("LEXIVA_ES_REAL_JSONL", WiktionaryEdition::Spanish, "caso");
}
