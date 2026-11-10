//! A dedicated, ignored stress test so normal unit test invocations remain fast.
//! The source corpus is generated incrementally: the test never creates a
//! multi-gigabyte string in memory.

use super::*;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::storage::DictionaryStore;

static NEXT_RUN: AtomicUsize = AtomicUsize::new(0);
const RECORDS: usize = 150_000;
const SELECTED: usize = RECORDS * 4 / 5;

struct TempFolder(PathBuf);

impl TempFolder {
    fn new() -> Self {
        let run = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lexiva-wiktextract-stress-{}-{run}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn generate_gzip(path: &Path) {
    let file = File::create(path).unwrap();
    let output = BufWriter::new(file);
    let mut writer = GzEncoder::new(output, Compression::fast());
    for index in 0..RECORDS {
        let lang = if index % 5 == 0 { "en" } else { "es" };
        let value = json!({
            "word": format!("palabra{index}"),
            "lang_code": lang,
            "pos": "noun",
            "senses": [{
                "glosses": [format!("A dictionary definition for word {index}")],
                "examples": [{"text": format!("Example number {index}")}]
            }],
            "etymology_text": "Synthetic test record",
            "extra": {"rank": index}
        });
        writeln!(writer, "{value}").unwrap();
    }
    writer.finish().unwrap();
}

#[cfg(target_os = "linux")]
fn peak_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn peak_rss_kib() -> Option<u64> {
    None
}

#[test]
#[ignore = "resource-intensive 150k JSONL records; run explicitly in CI"]
fn corpus_150k_records_import_cancel_reimport_and_measure_resources() {
    let tmp = TempFolder::new();
    let source_path = tmp.0.join("dictionary.jsonl.gz");
    let database_path = tmp.0.join("lexical.sqlite");

    let generated_at = Instant::now();
    generate_gzip(&source_path);
    let source_bytes = fs::metadata(&source_path).unwrap().len();

    let src = source_from_jsonl(
        &source_path,
        "stress-en-es",
        "Synthetic corpus resource test",
        WiktionaryEdition::English,
        Some("es"),
    )
    .unwrap();
    let mut store = DictionaryStore::open(&database_path).unwrap();
    store.upsert_source(&src).unwrap();

    let mut checkpoints = Vec::new();
    let started = Instant::now();
    let indexed = store
        .import_stream(
            &src.id,
            WiktextractReader::open(&source_path, &src, WiktionaryEdition::English).unwrap(),
            || false,
            |count| checkpoints.push(count),
        )
        .unwrap();
    let import_elapsed = started.elapsed();
    assert_eq!(indexed, SELECTED);
    assert_eq!(store.count_entries(&src.id).unwrap(), SELECTED as i64);
    assert_eq!(checkpoints.last(), Some(&SELECTED));
    assert!(checkpoints.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(checkpoints.len() >= 900);
    assert_eq!(store.staged_count(&src.id).unwrap(), 0);
    store.check_fts_index().unwrap();
    let unchanged = store.get_entry(&src.id, "jsonl-line:150000").unwrap();
    assert_eq!(
        unchanged.as_ref().map(|entry| entry.lemma.as_str()),
        Some("palabra149999")
    );
    let first_db_bytes = fs::metadata(&database_path).unwrap().len();

    // Cancel a replacement after at least two durable stage transactions;
    // the last published 120k records must remain intact.
    let progress = AtomicUsize::new(0);
    let cancelled = store.import_stream(
        &src.id,
        WiktextractReader::open(&source_path, &src, WiktionaryEdition::English).unwrap(),
        || progress.load(Ordering::Relaxed) >= 512,
        |count| {
            progress.store(count, Ordering::Relaxed);
        },
    );
    assert!(matches!(cancelled, Err(StoreError::Cancelled)));
    assert_eq!(store.count_entries(&src.id).unwrap(), SELECTED as i64);
    assert_eq!(store.staged_count(&src.id).unwrap(), 0);
    assert_eq!(store.get_entry(&src.id, "jsonl-line:150000").unwrap(), unchanged);

    // Reimport the same source and make sure FTS and published IDs remain
    // consistent; the input file must not be rewritten by either operation.
    let reimport_at = Instant::now();
    let reimported = store
        .import_stream(
            &src.id,
            WiktextractReader::open(&source_path, &src, WiktionaryEdition::English).unwrap(),
            || false,
            |_| {},
        )
        .unwrap();
    assert_eq!(reimported, SELECTED);
    assert_eq!(store.count_entries(&src.id).unwrap(), SELECTED as i64);
    assert_eq!(store.staged_count(&src.id).unwrap(), 0);
    assert_eq!(store.get_entry(&src.id, "jsonl-line:150000").unwrap(), unchanged);
    assert_eq!(source_bytes, fs::metadata(&source_path).unwrap().len());
    store.check_fts_index().unwrap();

    let final_db_bytes = fs::metadata(&database_path).unwrap().len();
    let rss = peak_rss_kib();
    eprintln!(
        "LEXIVA CORPUS: generated={} selected={} compressed_bytes={} db_first_bytes={} db_final_bytes={} peak_rss_kib={:?} generate_elapsed={:?} first_import={:?} reimport={:?}",
        RECORDS, SELECTED, source_bytes, first_db_bytes, final_db_bytes,
        rss, generated_at.elapsed(), import_elapsed, reimport_at.elapsed()
    );
    assert!(first_db_bytes > source_bytes);
    assert!(final_db_bytes > 0);
    // The exact peak is platform-dependent; a loose ceiling detects
    // accidentally collecting the entire multi-record corpus in memory.
    if let Some(kib) = rss {
        assert!(kib < 768 * 1024, "excessive RSS: {kib} KiB");
    }
}
