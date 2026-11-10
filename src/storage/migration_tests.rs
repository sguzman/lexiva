//! Existing v1 databases must migrate without losing entries or source IDs.

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::Value;

    use crate::model::{DictionarySource, LexicalEntry, SourceFormat};
    use crate::storage::{DictionaryStore, SCHEMA_VERSION, StoreError};

    fn source() -> DictionarySource {
        DictionarySource {
            id: "legacy".into(),
            title: "Existing v1 source".into(),
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
    fn migration_keeps_existing_v1_dictionary_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema_v1.sql"))
            .unwrap();
        let serialized = serde_json::to_string(&source()).unwrap();
        conn.execute(
            "INSERT INTO dictionary_sources (id, payload) VALUES ('legacy', ?1)",
            [serialized],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lexical_entries (
                source_id, source_entry_id, lemma, payload
             ) VALUES ('legacy', '1', 'casa', '{}')",
            [],
        )
        .unwrap();

        let store = DictionaryStore::initialize(conn).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(store.get_source("legacy").unwrap(), Some(source()));
        assert_eq!(store.count_entries("legacy").unwrap(), 1);
        assert_eq!(
            store
                .conn
                .query_row("SELECT count(*) FROM staging_imports", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        store.check_fts_index().unwrap();
    }

    #[test]
    fn rebuild_fts_recovers_live_entries() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source()).unwrap();
        store
            .replace_source_entries(
                "legacy",
                &[LexicalEntry {
                    source_id: "legacy".into(),
                    source_entry_id: "1".into(),
                    lemma: "árbol".into(),
                    headword_language: Some("es".into()),
                    definition_language: Some("en".into()),
                    part_of_speech: None,
                    senses: vec![],
                    pronunciations: vec![],
                    etymologies: vec![],
                    forms: vec![],
                    relations: vec![],
                    metadata: Value::Null,
                }],
            )
            .unwrap();
        assert_eq!(store.search_prefix("arb", None, 10).unwrap().len(), 1);
        store
            .conn
            .execute(
                "INSERT INTO lexical_fts(lexical_fts) VALUES ('delete-all')",
                [],
            )
            .unwrap();
        assert!(store.check_fts_index().is_err());
        store.rebuild_fts_index().unwrap();
        store.check_fts_index().unwrap();
        assert_eq!(store.search_prefix("arb", None, 10).unwrap().len(), 1);
    }

    #[test]
    fn damaged_cache_file_is_not_overwritten_automatically() {
        let path = std::env::temp_dir().join(format!(
            "lexiva-corrupt-{}.sqlite",
            std::process::id()
        ));
        let original = b"invalid sqlite database, preserve my bytes";
        std::fs::write(&path, original).unwrap();
        assert!(DictionaryStore::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn unsupported_future_version_is_not_modified() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(
            DictionaryStore::initialize(conn),
            Err(StoreError::UnsupportedSchema(99))
        ));
    }
}
