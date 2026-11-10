//! Durable, bounded staging for offline source imports.
//! Incomplete sessions never change the published index. A crash leaves
//! staging records that can be explicitly discarded after restart.

use rusqlite::params;

use super::{DictionaryStore, LexicalEntry, Result, StoreError};

pub const MAX_IMPORT_BATCH: usize = 128;
pub const MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;

/// Limit the serialized size *before* admitting an entry into the pending
/// batch. Count bytes without constructing a second giant JSON allocation.
struct LimitedCounter {
    bytes: usize,
    exceeded: bool,
}

impl std::io::Write for LimitedCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() > MAX_ENTRY_BYTES.saturating_sub(self.bytes) {
            self.exceeded = true;
            return Err(std::io::Error::other("dictionary entry exceeds size limit"));
        }
        self.bytes += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn ensure_entry_size(entry: &LexicalEntry) -> Result<()> {
    let mut counter = LimitedCounter {
        bytes: 0,
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut counter, entry) {
        if counter.exceeded {
            return Err(StoreError::Invalid("dictionary entry exceeds size limit"));
        }
        return Err(StoreError::Json(error));
    }
    Ok(())
}


/// Owns a single dictionary's staging lease on a DictionaryStore connection.
/// Dropping it cleans up normally; a process crash may leave staging rows,
/// but they cannot affect live searches or be published implicitly.
pub struct ImportSession<'a> {
    store: &'a mut DictionaryStore,
    source_id: String,
    staged: usize,
    published: bool,
}

impl DictionaryStore {
    /// Begin an import. A previous uncommitted stage must be explicitly
    /// discarded before restarting that source, to avoid deleting a
    /// concurrent process's active import.
    pub fn begin_import(&mut self, source_id: &str) -> Result<ImportSession<'_>> {
        let source = self
            .get_source(source_id)?
            .ok_or(StoreError::Invalid("dictionary source is not registered"))?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO staging_imports (source_id) VALUES (?1)",
            [source.id.as_str()],
        )?;
        tx.commit()?;
        Ok(ImportSession {
            store: self,
            source_id: source_id.to_owned(),
            staged: 0,
            published: false,
        })
    }

    /// Explicit cleanup for an abandoned import after process termination.
    /// The caller must ensure that no other process is actively staging
    /// the same source; no silent cleanup occurs on database open.
    pub fn discard_staged_import(&mut self, source_id: &str) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let deleted = tx.execute(
            "DELETE FROM staging_imports WHERE source_id = ?1",
            [source_id],
        )?;
        tx.commit()?;
        Ok(deleted > 0)
    }

    pub fn staged_count(&self, source_id: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM staging_entries WHERE source_id = ?1",
            [source_id],
            |row| row.get(0),
        )?)
    }

    /// Iterator-based ingestion with at most one fixed-size in-memory batch.
    /// The input iterator is expected to yield already parsed, bounded entries.
    /// A parser failure or cancellation discards the stage and retains the
    /// prior published dictionary. Successful publication is one transaction.
    pub fn import_stream<I, C, P>(
        &mut self,
        source_id: &str,
        entries: I,
        mut cancelled: C,
        mut on_progress: P,
    ) -> Result<usize>
    where
        I: IntoIterator<Item = Result<LexicalEntry>>,
        C: FnMut() -> bool,
        P: FnMut(usize),
    {
        let mut import = self.begin_import(source_id)?;
        let mut batch = Vec::with_capacity(MAX_IMPORT_BATCH);
        for item in entries {
            if cancelled() {
                return Err(StoreError::Cancelled);
            }
            let entry = item?;
            entry.validate().map_err(StoreError::Invalid)?;
            ensure_entry_size(&entry)?;
            batch.push(entry);
            if batch.len() == MAX_IMPORT_BATCH {
                import.stage_batch(&batch)?;
                batch.clear();
                on_progress(import.staged());
            }
        }
        if cancelled() {
            return Err(StoreError::Cancelled);
        }
        if !batch.is_empty() {
            import.stage_batch(&batch)?;
            on_progress(import.staged());
        }
        if cancelled() {
            return Err(StoreError::Cancelled);
        }
        import.publish()
    }
}

impl ImportSession<'_> {
    pub fn staged(&self) -> usize {
        self.staged
    }

    /// Durable one-batch write with a per-batch transaction. Prior staged
    /// rows survive a batch failure until this session is dropped/discarded.
    pub fn stage_batch(&mut self, entries: &[LexicalEntry]) -> Result<()> {
        if entries.len() > MAX_IMPORT_BATCH {
            return Err(StoreError::Invalid(
                "import batch exceeds configured maximum",
            ));
        }
        let source = self
            .store
            .get_source(&self.source_id)?
            .ok_or(StoreError::Invalid("dictionary source is missing"))?;
        let tx = self.store.conn.transaction()?;
        for entry in entries {
            entry.validate().map_err(StoreError::Invalid)?;
            if entry.source_id != self.source_id {
                return Err(StoreError::Invalid("entry belongs to a different source"));
            }
            ensure_entry_size(entry)?;
            let payload = serde_json::to_string(entry)?;
            if payload.len() > MAX_ENTRY_BYTES {
                return Err(StoreError::Invalid("import entry exceeds size limit"));
            }
            tx.execute(
                "INSERT INTO staging_entries (
                    source_id, source_entry_id, lemma,
                    headword_language, definition_language, part_of_speech, payload
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry.source_id,
                    entry.source_entry_id,
                    entry.lemma,
                    entry
                        .headword_language
                        .as_deref()
                        .or(source.headword_language.as_deref()),
                    entry
                        .definition_language
                        .as_deref()
                        .or(source.definition_language.as_deref()),
                    entry.part_of_speech,
                    payload,
                ],
            )?;
        }
        tx.commit()?;
        self.staged += entries.len();
        Ok(())
    }

    /// Atomic swap of a fully staged source into the published search index.
    /// It cannot publish an empty stage, avoiding accidental data loss.
    pub fn publish(mut self) -> Result<usize> {
        if self.staged == 0 {
            return Err(StoreError::EmptyImport);
        }
        let tx = self.store.conn.transaction()?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM staging_entries WHERE source_id = ?1",
            [self.source_id.as_str()],
            |row| row.get(0),
        )?;
        if count != self.staged as i64 {
            return Err(StoreError::Integrity(
                "staged entry count changed unexpectedly".to_owned(),
            ));
        }
        tx.execute(
            "DELETE FROM lexical_entries WHERE source_id = ?1",
            [self.source_id.as_str()],
        )?;
        tx.execute(
            "INSERT INTO lexical_entries (
                source_id, source_entry_id, lemma,
                headword_language, definition_language, part_of_speech, payload
             ) SELECT source_id, source_entry_id, lemma,
                      headword_language, definition_language, part_of_speech, payload
               FROM staging_entries WHERE source_id = ?1",
            [self.source_id.as_str()],
        )?;
        tx.execute(
            "DELETE FROM staging_imports WHERE source_id = ?1",
            [self.source_id.as_str()],
        )?;
        tx.commit()?;
        self.published = true;
        Ok(self.staged)
    }

    /// Explicit abort; Dropping a session also performs this cleanup.
    pub fn abort(self) {}
}

impl Drop for ImportSession<'_> {
    fn drop(&mut self) {
        if !self.published {
            // Best effort if a disk/IO error prevents immediate cleanup.
            // Reopen preserves live records, and discard_staged_import can
            // clear leftover staging data after exclusive access is ensured.
            let _ = self.store.discard_staged_import(&self.source_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::{Value, json};

    use super::*;
    use crate::model::{DictionarySource, SourceFormat};

    fn source(id: &str) -> DictionarySource {
        DictionarySource {
            id: id.into(),
            title: format!("Source: {id}"),
            format: SourceFormat::Wiktextract,
            origin: None,
            headword_language: Some("es".into()),
            definition_language: Some("en".into()),
            license: Some("CC-BY-SA-4.0".into()),
            attribution: None,
            metadata: Value::Null,
        }
    }

    fn entry(id: usize, source: &str) -> LexicalEntry {
        LexicalEntry {
            source_id: source.into(),
            source_entry_id: id.to_string(),
            lemma: format!("palabra{id}"),
            headword_language: Some("es".into()),
            definition_language: Some("en".into()),
            part_of_speech: None,
            senses: vec![],
            pronunciations: vec![],
            etymologies: vec![],
            forms: vec![],
            relations: vec![],
            metadata: json!({"record": id}),
        }
    }

    #[test]
    fn abandoned_on_disk_stage_does_not_replace_live_words() {
        static NEXT_DB: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT_DB.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lexiva-stage-{}-{number}.sqlite",
            std::process::id()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());

        {
            let mut first = DictionaryStore::open(&path).unwrap();
            first.upsert_source(&source("words")).unwrap();
            first
                .replace_source_entries("words", &[entry(999, "words")])
                .unwrap();
            // Simulate a process that dies between committed batches and
            // does not run ImportSession::drop.
            first
                .conn
                .execute(
                    "INSERT INTO staging_imports (source_id) VALUES ('words')",
                    [],
                )
                .unwrap();
            first
                .conn
                .execute(
                    "INSERT INTO staging_entries (
                        source_id, source_entry_id, lemma, payload
                     ) VALUES ('words', '1', 'palabra1', '{}')",
                    [],
                )
                .unwrap();
        }

        let mut reopened = DictionaryStore::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), 2);
        assert_eq!(reopened.count_entries("words").unwrap(), 1);
        assert_eq!(reopened.staged_count("words").unwrap(), 1);
        assert!(reopened.get_entry("words", "999").unwrap().is_some());
        // A concurrent/new import cannot steal or implicitly publish it.
        assert!(reopened.begin_import("words").is_err());
        assert!(reopened.discard_staged_import("words").unwrap());
        assert_eq!(reopened.staged_count("words").unwrap(), 0);
        assert_eq!(reopened.count_entries("words").unwrap(), 1);
        reopened.check_catalog_integrity().unwrap();

        let mut replacement = reopened.begin_import("words").unwrap();
        replacement.stage_batch(&[entry(2, "words")]).unwrap();
        replacement.publish().unwrap();
        assert_eq!(reopened.count_entries("words").unwrap(), 1);
        assert!(reopened.get_entry("words", "2").unwrap().is_some());
        reopened.check_fts_index().unwrap();
    }

    #[test]
    fn large_stream_is_bounded_and_published_atomically() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        db.replace_source_entries("words", &[entry(9999, "words")])
            .unwrap();
        let mut progress = Vec::new();
        let count = db
            .import_stream(
                "words",
                (0..1000).map(|i| Ok(entry(i, "words"))),
                || false,
                |count| progress.push(count),
            )
            .unwrap();
        assert_eq!(count, 1000);
        assert_eq!(progress.last(), Some(&1000));
        assert!(progress.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(db.count_entries("words").unwrap(), 1000);
        assert!(db.get_entry("words", "9999").unwrap().is_none());
        assert_eq!(db.staged_count("words").unwrap(), 0);
        db.check_fts_index().unwrap();
    }

    #[test]
    fn incomplete_stage_does_not_change_live_entries() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        let old = entry(999, "words");
        db.replace_source_entries("words", std::slice::from_ref(&old))
            .unwrap();
        {
            let mut pending = db.begin_import("words").unwrap();
            pending.stage_batch(&[entry(1, "words")]).unwrap();
            assert_eq!(pending.store.count_entries("words").unwrap(), 1);
            assert_eq!(
                pending.store.get_entry("words", "999").unwrap(),
                Some(old.clone())
            );
            assert_eq!(pending.store.staged_count("words").unwrap(), 1);
        }
        assert_eq!(db.count_entries("words").unwrap(), 1);
        assert_eq!(db.staged_count("words").unwrap(), 0);
        assert_eq!(db.get_entry("words", "999").unwrap(), Some(old));
    }

    #[test]
    fn cancelled_import_does_not_replace_existing_data() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        db.replace_source_entries("words", &[entry(999, "words")])
            .unwrap();
        let seen = AtomicUsize::new(0);
        let result = db.import_stream(
            "words",
            (0..500).map(|i| Ok(entry(i, "words"))),
            || seen.load(Ordering::Relaxed) >= 256,
            |count| {
                seen.store(count, Ordering::Relaxed);
            },
        );
        assert!(matches!(result, Err(StoreError::Cancelled)));
        assert_eq!(db.count_entries("words").unwrap(), 1);
        assert_eq!(db.staged_count("words").unwrap(), 0);
        assert!(db.get_entry("words", "999").unwrap().is_some());
    }

    #[test]
    fn parser_error_duplicate_and_empty_stages_never_publish() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        db.replace_source_entries("words", &[entry(999, "words")])
            .unwrap();
        let result = db.import_stream(
            "words",
            [
                Ok(entry(1, "words")),
                Err(StoreError::Invalid("source parse error")),
            ],
            || false,
            |_| {},
        );
        assert!(matches!(result, Err(StoreError::Invalid(_))));
        assert_eq!(db.staged_count("words").unwrap(), 0);
        let result = db.import_stream(
            "words",
            [Ok(entry(1, "words")), Ok(entry(1, "words"))],
            || false,
            |_| {},
        );
        assert!(result.is_err());
        assert_eq!(db.count_entries("words").unwrap(), 1);
        assert_eq!(db.staged_count("words").unwrap(), 0);
        let result = db.import_stream("words", std::iter::empty(), || false, |_| {});
        assert!(matches!(result, Err(StoreError::EmptyImport)));
        assert!(db.get_entry("words", "999").unwrap().is_some());
    }

    #[test]
    fn malformed_and_cross_source_input_are_rejected_without_publication() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        let mut wrong = entry(1, "another-source");
        let result = db.import_stream("words", [Ok(wrong.clone())], || false, |_| {});
        assert!(matches!(result, Err(StoreError::Invalid(_))));
        wrong.source_id = "words".into();
        wrong.source_entry_id = String::new();
        let result = db.import_stream("words", [Ok(wrong)], || false, |_| {});
        assert!(matches!(result, Err(StoreError::Invalid(_))));
        assert_eq!(db.staged_count("words").unwrap(), 0);
    }

    #[test]
    fn explicit_discard_preserves_live_data() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        db.replace_source_entries("words", &[entry(999, "words")])
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO staging_imports (source_id) VALUES ('words')",
                [],
            )
            .unwrap();
        db.conn
            .execute(
                "INSERT INTO staging_entries (
                    source_id, source_entry_id, lemma, payload
                 ) VALUES ('words', '0', 'palabra0', '{}')",
                [],
            )
            .unwrap();
        assert_eq!(db.staged_count("words").unwrap(), 1);
        assert!(db.discard_staged_import("words").unwrap());
        assert!(!db.discard_staged_import("words").unwrap());
        assert_eq!(db.staged_count("words").unwrap(), 0);
        assert_eq!(db.count_entries("words").unwrap(), 1);
    }

    #[test]
    fn oversized_record_is_rejected_before_staging() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        db.replace_source_entries("words", &[entry(999, "words")])
            .unwrap();
        let mut oversized = entry(1, "words");
        oversized.metadata = Value::String("x".repeat(MAX_ENTRY_BYTES));
        let result = db.import_stream("words", [Ok(oversized)], || false, |_| {});
        assert!(matches!(result, Err(StoreError::Invalid(_))));
        assert_eq!(db.count_entries("words").unwrap(), 1);
        assert_eq!(db.staged_count("words").unwrap(), 0);
        assert!(db.get_entry("words", "999").unwrap().is_some());
    }

    #[test]
    fn large_batch_is_rejected_without_side_effects() {
        let mut db = DictionaryStore::in_memory().unwrap();
        db.upsert_source(&source("words")).unwrap();
        let mut session = db.begin_import("words").unwrap();
        let too_many: Vec<_> = (0..=MAX_IMPORT_BATCH).map(|i| entry(i, "words")).collect();
        assert!(matches!(
            session.stage_batch(&too_many),
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(session.staged(), 0);
    }
}
