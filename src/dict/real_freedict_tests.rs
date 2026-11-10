//! Real package integration tests, executed only after the CI runner installs
//! Ubuntu's FreeDict spa-eng and eng-spa data packages. None is bundled.

use super::*;
use crate::storage::DictionaryStore;
use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "requires installed FreeDict packages and system gzip"]
fn installed_freedict_packages_agree_with_independent_gzip() {
    let directory = PathBuf::from("/usr/share/dictd");
    let discovered = discover_in(&[directory]).unwrap();
    for (base, from, to) in [
        ("freedict-spa-eng", "es", "en"),
        ("freedict-eng-spa", "en", "es"),
    ] {
        let files = discovered
            .iter()
            .find(|f| f.index.file_stem().is_some_and(|s| s == base))
            .unwrap_or_else(|| panic!("missing installed FreeDict package {base}"));
        assert!(files.compressed, "{base} should be a .dict.dz package");
        let source = metadata::source_from_files(files, base).unwrap();
        assert_eq!(source.headword_language.as_deref(), Some(from));
        assert_eq!(source.definition_language.as_deref(), Some(to));
        assert_eq!(source.format, crate::model::SourceFormat::Dict);
        assert!(source.origin.as_deref().unwrap().ends_with(".index"));
        assert_eq!(
            source.license, None,
            "per-dictionary license must be explicit"
        );

        let output = Command::new("gzip")
            .arg("-dc")
            .arg(&files.data)
            .output()
            .expect("system gzip must be installed");
        assert!(
            output.status.success(),
            "gzip independent decompression failed for {base}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let decompressed = output.stdout;
        assert!(decompressed.len() > 1000);

        // A full independent byte-for-byte validation of every lexical entry:
        // the standard gzip decoded article must equal Lexiva's random-access
        // dictzip output, including UTF-8 and any source punctuation/markup.
        let mut count = 0usize;
        let mut sample: Option<LexicalEntry> = None;
        for entry in DictReader::open(&source, files).unwrap() {
            let entry = entry.unwrap();
            let start = entry.metadata["dict_data_offset"].as_u64().unwrap() as usize;
            let length = entry.metadata["dict_article_bytes"].as_u64().unwrap() as usize;
            assert_eq!(
                entry.senses[0].glosses[0].as_bytes(),
                &decompressed[start..start + length],
                "incompatible DICT article offset at {} in {base}",
                entry.lemma
            );
            assert_eq!(entry.headword_language.as_deref(), Some(from));
            assert_eq!(entry.definition_language.as_deref(), Some(to));
            if sample.is_none()
                && entry.lemma.chars().all(|ch| ch.is_ascii_alphabetic())
                && entry.lemma.len() >= 3
            {
                sample = Some(entry.clone());
            }
            count += 1;
        }
        assert!(count >= 100, "package unexpectedly small: {base} ({count})");

        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source).unwrap();
        let inserted = store
            .import_stream(
                base,
                DictReader::open(&source, files).unwrap(),
                || false,
                |_| {},
            )
            .unwrap();
        assert_eq!(inserted, count);
        assert_eq!(store.count_entries(base).unwrap(), count as i64);
        let sample = sample.unwrap();
        assert_eq!(
            store.get_entry(base, &sample.source_entry_id).unwrap(),
            Some(sample.clone())
        );
        let results = store.search_prefix(&sample.lemma, Some(base), 100).unwrap();
        assert!(
            results
                .iter()
                .any(|item| item.source_entry_id == sample.source_entry_id)
        );
        store.check_fts_index().unwrap();
        eprintln!(
            "Real FreeDict QA: {base}, {count} entries, {} bytes decompressed, word {}",
            decompressed.len(),
            sample.lemma
        );
    }
}
