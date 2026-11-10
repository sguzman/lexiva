use super::*;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::storage::DictionaryStore;

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

// Original synthetic text shaped like documented Wiktextract output.
// No Wiktionary dump contents are bundled in these tests.
const EN_ES: &str = r#"{"word":"casa","lang":"Spanish","lang_code":"es","pos":"noun","senses":[{"glosses":["A dwelling","A person's home"],"raw_glosses":["(architecture) A dwelling"],"tags":["countable"],"examples":[{"text":"La casa es azul.","translation":"The house is blue.","ref":"Synthetic example"}]},{"glosses":["A family household"],"senseid":["synthetic:2"]}],"sounds":[{"ipa":"/ˈkasa/","tags":["Spain"]}],"forms":[{"form":"casas","tags":["plural"]}],"etymology_text":"From a Latin root.","synonyms":[{"word":"hogar"}],"future_field":{"nested":[1,{"key":"value"}]}}"#;
const ES_ES: &str = r#"{"word":"casa","lang":"español","lang_code":"es","pos":"sustantivo","senses":[{"glosses":["Edificio para vivir."],"examples":[{"text":"Su casa es pequeña."}]},{"glosses":["Familia o linaje."],"tags":["figurado"]}],"forms":[{"form":"casas"}]}"#;
const EN_EN: &str = r#"{"word":"home","lang":"English","lang_code":"en","pos":"noun","senses":[{"glosses":["The place where one lives."]}]}"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let ordinal = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lexiva-wiktextract-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn plain(&self, name: &str, lines: &str) -> PathBuf {
        let path = self.0.join(format!("{name}.jsonl"));
        fs::write(&path, lines).unwrap();
        path
    }

    fn compressed(&self, name: &str, lines: &str) -> PathBuf {
        let path = self.0.join(format!("{name}.jsonl.gz"));
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(lines.as_bytes()).unwrap();
        fs::write(&path, gz.finish().unwrap()).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn source(path: &Path, edition: WiktionaryEdition, filter: Option<&str>) -> DictionarySource {
    source_from_jsonl(path, "wiktionary", "Wiktionary extract", edition, filter).unwrap()
}

#[test]
fn english_edition_spanish_words_have_english_glosses_and_full_metadata() {
    let fixture = Fixture::new();
    let path = fixture.plain("en-extract", &format!("{EN_ES}\n"));
    let src = source(&path, WiktionaryEdition::English, Some("es"));
    let entries: Vec<_> = WiktextractReader::open(&path, &src, WiktionaryEdition::English)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let word = &entries[0];
    assert_eq!(word.lemma, "casa");
    assert_eq!(word.headword_language.as_deref(), Some("es"));
    assert_eq!(word.definition_language.as_deref(), Some("en"));
    assert_eq!(word.senses.len(), 2);
    assert_eq!(word.senses[0].glosses, ["A dwelling", "A person's home"]);
    assert_eq!(word.senses[1].glosses, ["A family household"]);
    assert_eq!(
        word.senses[0].examples[0].translation.as_deref(),
        Some("The house is blue.")
    );
    assert_eq!(
        word.senses[0].examples[0].reference.as_deref(),
        Some("Synthetic example")
    );
    assert_eq!(word.senses[0].labels, ["countable"]);
    assert_eq!(word.pronunciations[0].ipa.as_deref(), Some("/ˈkasa/"));
    assert_eq!(word.forms[0].spelling, "casas");
    assert_eq!(word.etymologies, ["From a Latin root."]);
    assert_eq!(word.relations[0].term, "hogar");
    assert_eq!(
        word.metadata["wiktextract_record"]["future_field"]["nested"][1]["key"],
        "value"
    );
    assert_eq!(
        word.metadata["wiktextract_record"]["senses"][1]["senseid"][0],
        "synthetic:2"
    );
    assert_eq!(word.source_entry_id, "jsonl-line:1");
}

#[test]
fn spanish_edition_definitions_are_spanish_even_for_spanish_headwords() {
    let fixture = Fixture::new();
    let path = fixture.compressed("es-extract", &format!("{ES_ES}\n{EN_EN}\n"));
    let src = source(&path, WiktionaryEdition::Spanish, Some("es"));
    assert_eq!(src.definition_language.as_deref(), Some("es"));
    let entries: Vec<_> = WiktextractReader::open(&path, &src, WiktionaryEdition::Spanish)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].lemma, "casa");
    assert_eq!(entries[0].headword_language.as_deref(), Some("es"));
    assert_eq!(entries[0].definition_language.as_deref(), Some("es"));
    assert_eq!(entries[0].senses[0].glosses, ["Edificio para vivir."]);
    assert_eq!(entries[0].senses[1].glosses, ["Familia o linaje."]);
    assert_eq!(entries[0].senses[0].examples[0].translation, None);
}

#[test]
fn gzip_and_plain_streams_produce_equivalent_entries() {
    let fixture = Fixture::new();
    let content = format!("{EN_ES}\n{EN_EN}\n{EN_ES}\n");
    let plain = fixture.plain("data", &content);
    let gz = fixture.compressed("data", &content);
    let src = source(&plain, WiktionaryEdition::English, Some("es"));
    let a: Vec<_> = WiktextractReader::open(&plain, &src, WiktionaryEdition::English)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    let b: Vec<_> = WiktextractReader::open(&gz, &src, WiktionaryEdition::English)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].source_entry_id, "jsonl-line:1");
    assert_eq!(a[1].source_entry_id, "jsonl-line:3");
    assert_ne!(a[0].source_entry_id, a[1].source_entry_id);
}

#[test]
fn source_edition_mismatch_and_wrong_format_are_rejected() {
    let fixture = Fixture::new();
    let path = fixture.plain("data", &format!("{EN_ES}\n"));
    let english = source(&path, WiktionaryEdition::English, Some("es"));
    assert!(WiktextractReader::open(&path, &english, WiktionaryEdition::Spanish).is_err());
    let mut wrong = english.clone();
    wrong.format = SourceFormat::Dict;
    assert!(WiktextractReader::open(&path, &wrong, WiktionaryEdition::English).is_err());
    let mut invalid = english;
    invalid.headword_language = Some("es_".to_owned());
    assert!(WiktextractReader::open(&path, &invalid, WiktionaryEdition::English).is_err());
    assert!(source_from_jsonl(&path, "", "test", WiktionaryEdition::English, None).is_err());
}

#[test]
fn streaming_gzip_into_sqlite_is_source_scoped_and_reports_batches() {
    let fixture = Fixture::new();
    let mut data = String::new();
    for index in 0..400 {
        data.push_str(&format!(
            r#"{{"word":"palabra{index}","lang_code":"es","pos":"noun","senses":[{{"glosses":["Word {index}"]}}]}}"#
        ));
        data.push('\n');
    }
    let path = fixture.compressed("batch", &data);
    let src = source(&path, WiktionaryEdition::English, Some("es"));
    let mut store = DictionaryStore::in_memory().unwrap();
    store.upsert_source(&src).unwrap();
    let mut progress = Vec::new();
    let imported = store
        .import_stream(
            &src.id,
            WiktextractReader::open(&path, &src, WiktionaryEdition::English).unwrap(),
            || false,
            |count| progress.push(count),
        )
        .unwrap();
    assert_eq!(imported, 400);
    assert_eq!(progress, [128, 256, 384, 400]);
    assert_eq!(store.count_entries(&src.id).unwrap(), 400);
    let entries = store.search_prefix("palabra3", Some(&src.id), 100).unwrap();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .all(|entry| entry.headword_language.as_deref() == Some("es"))
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry.definition_language.as_deref() == Some("en"))
    );
    store.check_fts_index().unwrap();
}

#[test]
fn malformed_json_rolls_back_previously_published_dictionary() {
    let fixture = Fixture::new();
    let good_path = fixture.plain("good", &format!("{EN_ES}\n"));
    let bad_path = fixture.plain("bad", &format!("{EN_ES}\nnot-json\n"));
    let src = source(&good_path, WiktionaryEdition::English, Some("es"));
    let mut store = DictionaryStore::in_memory().unwrap();
    store.upsert_source(&src).unwrap();
    store
        .import_stream(
            &src.id,
            WiktextractReader::open(&good_path, &src, WiktionaryEdition::English).unwrap(),
            || false,
            |_| {},
        )
        .unwrap();
    let before = store.get_entry(&src.id, "jsonl-line:1").unwrap();
    let result = store.import_stream(
        &src.id,
        WiktextractReader::open(&bad_path, &src, WiktionaryEdition::English).unwrap(),
        || false,
        |_| {},
    );
    assert!(result.is_err());
    assert_eq!(store.count_entries(&src.id).unwrap(), 1);
    assert_eq!(store.get_entry(&src.id, "jsonl-line:1").unwrap(), before);
    assert_eq!(store.staged_count(&src.id).unwrap(), 0);
}

#[test]
fn oversized_decompressed_line_is_rejected_before_json_allocation() {
    let fixture = Fixture::new();
    let path = fixture.compressed("oversized", &"x".repeat(MAX_JSONL_LINE_BYTES as usize + 16));
    let src = source(&path, WiktionaryEdition::Spanish, Some("es"));
    let mut reader = WiktextractReader::open(&path, &src, WiktionaryEdition::Spanish).unwrap();
    assert!(reader.next().unwrap().is_err());
    assert!(reader.next().is_none());
}

#[test]
fn missing_language_code_and_invalid_word_stop_the_stream() {
    let fixture = Fixture::new();
    let path = fixture.plain("missing", "{\"word\":\"casa\"}\n");
    let src = source(&path, WiktionaryEdition::English, None);
    let mut reader = WiktextractReader::open(&path, &src, WiktionaryEdition::English).unwrap();
    assert!(reader.next().unwrap().is_err());
    let path = fixture.plain("bad-word", "{\"word\":42,\"lang_code\":\"es\"}\n");
    let mut reader = WiktextractReader::open(&path, &src, WiktionaryEdition::English).unwrap();
    assert!(reader.next().unwrap().is_err());
}

#[test]
fn raw_glosses_fallback_and_sound_links_are_preserved() {
    let fixture = Fixture::new();
    let data = r#"{"word":"niño","lang_code":"es","pos":"noun","senses":[{"raw_glosses":["Child or young person."],"tags":["informal"],"examples":[{"text":"Un niño.","english":"A child."}]}],"sounds":[{"ogg_url":"https://example.invalid/voice.ogg","tags":["Mexico"]}],"etymology_text":"Etymology text."}"#;
    let path = fixture.plain("more-fields", &format!("{data}\n"));
    let src = source(&path, WiktionaryEdition::English, Some("es"));
    let records: Vec<_> = WiktextractReader::open(&path, &src, WiktionaryEdition::English)
        .unwrap()
        .collect::<StoreResult<Vec<_>>>()
        .unwrap();
    assert_eq!(records[0].senses[0].glosses, ["Child or young person."]);
    assert_eq!(
        records[0].senses[0].examples[0].translation.as_deref(),
        Some("A child.")
    );
    assert_eq!(
        records[0].pronunciations[0].audio.as_deref(),
        Some("https://example.invalid/voice.ogg")
    );
    assert_eq!(records[0].pronunciations[0].labels, ["Mexico"]);
}
