//! Local, format-neutral SQLite catalog and word index.
//! Backend is not yet wired to the GUI. Derived indices must be rebuildable.

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{DictionarySource, LexicalEntry};

pub const SCHEMA_VERSION: i64 = 1;
const MAX_QUERY_RESULTS: usize = 100;

#[derive(Debug)]
pub enum StoreError {
    Database(rusqlite::Error),
    Json(serde_json::Error),
    Invalid(&'static str),
    UnsupportedSchema(i64),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "SQLite error: {error}"),
            Self::Json(error) => write!(f, "dictionary record serialization error: {error}"),
            Self::Invalid(reason) => write!(f, "invalid dictionary data: {reason}"),
            Self::UnsupportedSchema(version) => {
                write!(
                    f,
                    "database schema version {version} is newer than Lexiva supports"
                )
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct DictionaryStore {
    conn: Connection,
}

impl DictionaryStore {
    pub fn open(path: &Path) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }

    #[cfg(test)]
    fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                conn.execute_batch(include_str!("schema.sql"))?;
            }
            SCHEMA_VERSION => (),
            other => return Err(StoreError::UnsupportedSchema(other)),
        }
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn upsert_source(&self, source: &DictionarySource) -> Result<()> {
        source.validate().map_err(StoreError::Invalid)?;
        let payload = serde_json::to_string(source)?;
        self.conn.execute(
            "INSERT INTO dictionary_sources (id, payload) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            params![source.id, payload],
        )?;
        Ok(())
    }

    pub fn get_source(&self, source_id: &str) -> Result<Option<DictionarySource>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM dictionary_sources WHERE id = ?1",
                [source_id],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|payload| serde_json::from_str(&payload).map_err(Into::into))
            .transpose()
    }

    /// Atomically replace all indexed entries of a source.
    ///
    /// This intentionally accepts a bounded batch. Large streaming imports
    /// will use a separate staging table before publication (LEX-023).
    pub fn replace_source_entries(
        &mut self,
        source_id: &str,
        entries: &[LexicalEntry],
    ) -> Result<()> {
        let source = self
            .get_source(source_id)?
            .ok_or(StoreError::Invalid("dictionary source is not registered"))?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM lexical_entries WHERE source_id = ?1",
            [source_id],
        )?;
        for entry in entries {
            entry.validate().map_err(StoreError::Invalid)?;
            if entry.source_id != source_id {
                return Err(StoreError::Invalid("entry belongs to a different source"));
            }
            let payload = serde_json::to_string(entry)?;
            tx.execute(
                "INSERT INTO lexical_entries (
                    source_id, source_entry_id, lemma, headword_language,
                    definition_language, part_of_speech, payload
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
                    payload
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_entry(
        &self,
        source_id: &str,
        source_entry_id: &str,
    ) -> Result<Option<LexicalEntry>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM lexical_entries
                 WHERE source_id = ?1 AND source_entry_id = ?2",
                params![source_id, source_entry_id],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|payload| serde_json::from_str(&payload).map_err(Into::into))
            .transpose()
    }

    pub fn count_entries(&self, source_id: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM lexical_entries WHERE source_id = ?1",
            [source_id],
            |row| row.get(0),
        )?)
    }

    /// Basic source-scoped FTS prefix lookup. Language filters and ranking are
    /// separate search-engine work in M05/M06. Query syntax is always escaped.
    pub fn search_prefix(
        &self,
        query: &str,
        source_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LexicalEntry>> {
        let query = query.trim();
        if limit == 0 || !query.chars().any(char::is_alphanumeric) {
            return Ok(Vec::new());
        }
        let prefix = format!("\"{}\"*", query.replace('"', "\"\""));
        let mut stmt = self.conn.prepare(
            "SELECT e.payload FROM lexical_fts
             JOIN lexical_entries e ON e.id = lexical_fts.rowid
             WHERE lexical_fts MATCH ?1
               AND (?2 IS NULL OR e.source_id = ?2)
             ORDER BY e.lemma COLLATE NOCASE, e.source_id, e.id
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![prefix, source_id, limit.min(MAX_QUERY_RESULTS) as i64],
            |row| row.get::<_, String>(0),
        )?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(serde_json::from_str(&row?)?);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::model::{Sense, SourceFormat};

    fn source(id: &str, definition_language: &str) -> DictionarySource {
        DictionarySource {
            id: id.into(),
            title: format!("Source: {id}"),
            format: SourceFormat::Dict,
            origin: Some(format!("/usr/share/dictd/{id}.index")),
            headword_language: Some("es".into()),
            definition_language: Some(definition_language.into()),
            license: Some("GPL-3.0-or-later".into()),
            attribution: Some("Original FreeDict contributors".into()),
            metadata: json!({"revision": 12, "original": ["one", "two"]}),
        }
    }

    fn entry(source_id: &str, entry_id: &str, lemma: &str) -> LexicalEntry {
        LexicalEntry {
            source_id: source_id.into(),
            source_entry_id: entry_id.into(),
            lemma: lemma.into(),
            headword_language: Some("es".into()),
            definition_language: None,
            part_of_speech: Some("noun".into()),
            senses: vec![Sense {
                number: Some("1".into()),
                definition_language: Some("en".into()),
                glosses: vec!["house".into(), "home".into()],
                labels: vec![],
                examples: vec![],
                children: vec![],
            }],
            pronunciations: vec![],
            etymologies: vec![],
            forms: vec![],
            relations: vec![],
            metadata: json!({"unmapped-key": {"a": 4}}),
        }
    }

    #[test]
    fn schema_created_and_source_provenance_round_trips() {
        let store = DictionaryStore::in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        let s = source("fre-es-en", "en");
        store.upsert_source(&s).unwrap();
        assert_eq!(store.get_source(&s.id).unwrap(), Some(s));
        assert_eq!(store.get_source("missing").unwrap(), None);
    }

    #[test]
    fn indexed_entries_preserve_structure_and_source_identity() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("es-en", "en")).unwrap();
        store.upsert_source(&source("es-es", "es")).unwrap();
        let a = entry("es-en", "homograph:1", "casa");
        let b = entry("es-en", "homograph:2", "casa");
        let c = entry("es-es", "lemma:1", "casa");
        store
            .replace_source_entries("es-en", &[a.clone(), b.clone()])
            .unwrap();
        store.replace_source_entries("es-es", &[c.clone()]).unwrap();
        assert_eq!(store.count_entries("es-en").unwrap(), 2);
        assert_eq!(store.get_entry("es-en", "homograph:1").unwrap(), Some(a));
        assert_eq!(store.get_entry("es-en", "homograph:2").unwrap(), Some(b));
        assert_eq!(store.search_prefix("cas", None, 10).unwrap().len(), 3);
        let scoped = store.search_prefix("cas", Some("es-en"), 10).unwrap();
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|item| item.source_id == "es-en"));
        assert_eq!(
            store.search_prefix("cas", Some("es-es"), 10).unwrap(),
            vec![c]
        );
    }

    #[test]
    fn replacement_is_atomic_and_removes_stale_fts_entries() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("es-en", "en")).unwrap();
        let old = entry("es-en", "old", "árbol");
        store
            .replace_source_entries("es-en", &[old.clone()])
            .unwrap();

        let first = entry("es-en", "same", "casa");
        let duplicate = entry("es-en", "same", "casita");
        assert!(
            store
                .replace_source_entries("es-en", &[first, duplicate])
                .is_err()
        );
        assert_eq!(store.count_entries("es-en").unwrap(), 1);
        assert_eq!(store.get_entry("es-en", "old").unwrap(), Some(old));

        let newer = entry("es-en", "new", "hogar");
        store
            .replace_source_entries("es-en", &[newer.clone()])
            .unwrap();
        assert!(store.search_prefix("ár", None, 10).unwrap().is_empty());
        assert_eq!(store.search_prefix("hog", None, 10).unwrap(), vec![newer]);
        store.replace_source_entries("es-en", &[]).unwrap();
        assert_eq!(store.count_entries("es-en").unwrap(), 0);
        assert!(store.search_prefix("hog", None, 10).unwrap().is_empty());
    }

    #[test]
    fn query_is_escaped_and_empty_queries_are_harmless() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("es-en", "en")).unwrap();
        store
            .replace_source_entries("es-en", &[entry("es-en", "1", "casa")])
            .unwrap();
        assert_eq!(store.search_prefix("", None, 10).unwrap(), Vec::new());
        assert_eq!(store.search_prefix("\"", None, 10).unwrap(), Vec::new());
        assert_eq!(store.search_prefix("casa", None, 0).unwrap(), Vec::new());
        assert!(store.search_prefix("casa\" OR \"*", None, 10).is_ok());
        assert_eq!(store.search_prefix("cas", None, 10).unwrap().len(), 1);
    }

    #[test]
    fn unsupported_schema_is_not_mutated() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 40).unwrap();
        let result = DictionaryStore::initialize(conn);
        assert!(matches!(result, Err(StoreError::UnsupportedSchema(40))));
    }

    #[test]
    fn source_metadata_may_be_null_without_guessing_language() {
        let mut store = DictionaryStore::in_memory().unwrap();
        let mut unknown = source("unknown", "es");
        unknown.definition_language = None;
        unknown.headword_language = None;
        unknown.metadata = Value::Null;
        store.upsert_source(&unknown).unwrap();
        let mut item = entry("unknown", "1", "azul");
        item.headword_language = None;
        item.senses.clear();
        store
            .replace_source_entries("unknown", &[item.clone()])
            .unwrap();
        assert_eq!(store.get_entry("unknown", "1").unwrap(), Some(item));
    }
}
