//! Indexed, bounded search with stable source and homograph attribution.
//! No source is silently merged: every hit retains its dictionary ID.

use std::collections::HashSet;

use rusqlite::params;

use super::{DictionaryStore, Result};
use crate::model::{DictionarySource, LexicalEntry};

const MAX_SEARCH_HITS: usize = 100;

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchFilters<'a> {
    pub source_id: Option<&'a str>,
    pub headword_language: Option<&'a str>,
    pub definition_language: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub entry: LexicalEntry,
    pub source: DictionarySource,
    pub exact: bool,
}

impl DictionaryStore {
    pub fn list_sources(&self) -> Result<Vec<DictionarySource>> {
        let mut statement = self
            .conn
            .prepare("SELECT payload FROM dictionary_sources ORDER BY id")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let payload = row?;
            Ok(serde_json::from_str(&payload)?)
        })
        .collect()
    }

    /// FTS5 removes Latin accents for search. Literal exact spellings are
    /// ranked before prefixes, then source and entry IDs break ties.
    /// All MATCH input is quoted, never interpreted as FTS query syntax.
    pub fn search_ranked(
        &self,
        query: &str,
        filters: SearchFilters<'_>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let query = query.trim();
        if limit == 0 || !query.chars().any(char::is_alphanumeric) {
            return Ok(Vec::new());
        }

        let quoted = format!("\"{}\"", query.replace('"', "\"\""));
        let mut collected = Vec::new();
        let mut seen = HashSet::new();

        for expression in [&quoted, &format!("{quoted}*")] {
            let mut statement = self.conn.prepare(
                "SELECT e.payload, s.payload
                 FROM lexical_fts
                 JOIN lexical_entries e ON e.id = lexical_fts.rowid
                 JOIN dictionary_sources s ON s.id = e.source_id
                 WHERE lexical_fts MATCH ?1
                   AND (?2 IS NULL OR e.source_id = ?2)
                   AND (?3 IS NULL OR e.headword_language = ?3)
                   AND (?4 IS NULL OR e.definition_language = ?4)
                 ORDER BY e.lemma COLLATE NOCASE, e.source_id, e.source_entry_id, e.id
                 LIMIT ?5",
            )?;
            let rows = statement.query_map(
                params![
                    expression,
                    filters.source_id,
                    filters.headword_language,
                    filters.definition_language,
                    MAX_SEARCH_HITS as i64
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            for row in rows {
                let (entry_json, source_json) = row?;
                let entry: LexicalEntry = serde_json::from_str(&entry_json)?;
                let source: DictionarySource = serde_json::from_str(&source_json)?;
                if seen.insert((entry.source_id.clone(), entry.source_entry_id.clone())) {
                    let exact = entry.lemma.to_lowercase() == query.to_lowercase();
                    collected.push(SearchHit {
                        entry,
                        source,
                        exact,
                    });
                }
            }
        }
        collected.sort_by(|left, right| {
            right
                .exact
                .cmp(&left.exact)
                .then_with(|| {
                    left.entry
                        .lemma
                        .to_lowercase()
                        .cmp(&right.entry.lemma.to_lowercase())
                })
                .then_with(|| left.entry.source_id.cmp(&right.entry.source_id))
                .then_with(|| left.entry.source_entry_id.cmp(&right.entry.source_entry_id))
        });
        collected.truncate(limit.min(MAX_SEARCH_HITS));
        Ok(collected)
    }

    /// Offer a short, source-filtered edit-distance suggestion list only
    /// after exact/prefix lookup returned nothing. FTS prefix candidate
    /// lookup is bounded; no full-corpus Levenshtein scan occurs.
    pub fn suggest_words(
        &self,
        query: &str,
        filters: SearchFilters<'_>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let query = query.trim();
        let folded = fold_latin(query);
        let characters: Vec<char> = folded.chars().collect();
        if limit == 0 || characters.len() < 3 || characters.len() > 64 {
            return Ok(Vec::new());
        }
        if !characters.iter().all(|ch| ch.is_alphabetic()) {
            return Ok(Vec::new());
        }

        let mut suggestions = Vec::<(usize, String)>::new();
        let mut seen = HashSet::new();
        // Search a narrower prefix first, then a one-character fallback.
        for prefix_length in [3usize, 1] {
            if !suggestions.is_empty() {
                break;
            }
            let prefix: String = characters
                .iter()
                .take(prefix_length.min(characters.len()))
                .collect();
            let expression = format!("\"{}\"*", prefix.replace('"', "\"\""));
            let mut statement = self.conn.prepare(
                "SELECT DISTINCT e.lemma FROM lexical_fts
                 JOIN lexical_entries e ON e.id = lexical_fts.rowid
                 WHERE lexical_fts MATCH ?1
                   AND (?2 IS NULL OR e.source_id = ?2)
                   AND (?3 IS NULL OR e.headword_language = ?3)
                   AND (?4 IS NULL OR e.definition_language = ?4)
                 ORDER BY e.lemma COLLATE NOCASE
                 LIMIT 512",
            )?;
            let rows = statement.query_map(
                params![
                    expression,
                    filters.source_id,
                    filters.headword_language,
                    filters.definition_language
                ],
                |row| row.get::<_, String>(0),
            )?;
            let allowed = if characters.len() >= 6 { 2 } else { 1 };
            for row in rows {
                let lemma = row?;
                let normalized = fold_latin(&lemma);
                let distance = bounded_distance(&characters, &normalized, allowed);
                if distance.is_some_and(|distance| distance != 0) && seen.insert(lemma.clone()) {
                    suggestions.push((distance.unwrap_or(allowed), lemma));
                }
            }
        }
        suggestions.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| fold_latin(&left.1).cmp(&fold_latin(&right.1)))
                .then_with(|| left.1.cmp(&right.1))
        });
        Ok(suggestions
            .into_iter()
            .take(limit.min(8))
            .map(|(_, word)| word)
            .collect())
    }
}


/// FTS5's Latin accent handling is approximated only for suggestion ranking.
/// Stored source spelling is never rewritten or de-accented.
fn fold_latin(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Return at most max+1 cells of interest per row: maximum 64 characters.
/// Used only with bounded FTS candidates, never with the entire lexicon.
fn bounded_distance(left: &[char], right: &str, max: usize) -> Option<usize> {
    let rhs: Vec<char> = right.chars().collect();
    if left.len().abs_diff(rhs.len()) > max {
        return None;
    }
    let mut previous: Vec<usize> = (0..=rhs.len()).collect();
    for (i, a) in left.iter().enumerate() {
        let mut current = vec![i + 1; rhs.len() + 1];
        for (j, b) in rhs.iter().enumerate() {
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + usize::from(a != b));
        }
        if current.iter().copied().min().unwrap_or(max + 1) > max {
            return None;
        }
        previous = current;
    }
    (previous[rhs.len()] <= max).then_some(previous[rhs.len()])
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::model::{Sense, SourceFormat};

    fn source(id: &str, definition_language: &str) -> DictionarySource {
        DictionarySource {
            id: id.to_owned(),
            title: format!("{id} definitions"),
            format: SourceFormat::Dict,
            origin: None,
            headword_language: Some("es".to_owned()),
            definition_language: Some(definition_language.to_owned()),
            license: None,
            attribution: Some("Synthetic test source".to_owned()),
            metadata: Value::Null,
        }
    }

    fn entry(source: &str, id: &str, word: &str, meaning: &str) -> LexicalEntry {
        LexicalEntry {
            source_id: source.to_owned(),
            source_entry_id: id.to_owned(),
            lemma: word.to_owned(),
            headword_language: Some("es".to_owned()),
            definition_language: None,
            part_of_speech: Some("noun".to_owned()),
            senses: vec![Sense {
                number: Some("1".to_owned()),
                definition_language: Some("en".to_owned()),
                glosses: vec![meaning.to_owned()],
                labels: vec![],
                examples: vec![],
                children: vec![],
            }],
            pronunciations: vec![],
            etymologies: vec![],
            forms: vec![],
            relations: vec![],
            metadata: Value::Null,
        }
    }

    #[test]
    fn exact_comes_before_prefix_with_stable_homograph_identity() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("a", "en")).unwrap();
        store.upsert_source(&source("b", "en")).unwrap();
        let a = [
            entry("a", "2", "casa", "home"),
            entry("a", "1", "casa", "house"),
            entry("a", "3", "casamiento", "wedding"),
        ];
        store.replace_source_entries("a", &a).unwrap();
        store
            .replace_source_entries("b", &[entry("b", "1", "casa", "dwelling")])
            .unwrap();
        let results = store
            .search_ranked("cas", SearchFilters::default(), 10)
            .unwrap();
        assert_eq!(results.len(), 4);
        let exact = store
            .search_ranked("CASA", SearchFilters::default(), 10)
            .unwrap();
        assert_eq!(exact.len(), 4);
        assert!(exact.iter().take(3).all(|hit| hit.exact));
        assert!(!exact[3].exact);
        assert_eq!(exact[3].entry.lemma, "casamiento");
        assert_eq!(
            exact
                .iter()
                .take(3)
                .map(|hit| (
                    hit.entry.source_id.as_str(),
                    hit.entry.source_entry_id.as_str()
                ))
                .collect::<Vec<_>>(),
            [("a", "1"), ("a", "2"), ("b", "1")]
        );
        assert_eq!(exact[0].source.title, "a definitions");
    }

    #[test]
    fn accent_insensitive_index_search_retains_original_unicode() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("a", "en")).unwrap();
        store
            .replace_source_entries(
                "a",
                &[
                    entry("a", "1", "árbol", "tree"),
                    entry("a", "2", "niño", "boy"),
                    entry("a", "3", "pingüino", "penguin"),
                ],
            )
            .unwrap();
        for (query, expected) in [
            ("arbo", "árbol"),
            ("ARBOL", "árbol"),
            ("nino", "niño"),
            ("pingu", "pingüino"),
        ] {
            let hits = store
                .search_ranked(query, SearchFilters::default(), 10)
                .unwrap();
            assert_eq!(hits[0].entry.lemma, expected, "query {query}");
        }
    }

    #[test]
    fn source_and_languages_are_independent_filters() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("a", "en")).unwrap();
        store.upsert_source(&source("b", "es")).unwrap();
        store
            .replace_source_entries("a", &[entry("a", "1", "casa", "house")])
            .unwrap();
        store
            .replace_source_entries("b", &[entry("b", "1", "casa", "edificio")])
            .unwrap();
        assert_eq!(store.list_sources().unwrap().len(), 2);
        for (filter, expected) in [
            (
                SearchFilters {
                    source_id: Some("a"),
                    ..Default::default()
                },
                "a",
            ),
            (
                SearchFilters {
                    definition_language: Some("es"),
                    ..Default::default()
                },
                "b",
            ),
            (
                SearchFilters {
                    headword_language: Some("es"),
                    definition_language: Some("en"),
                    ..Default::default()
                },
                "a",
            ),
        ] {
            let hits = store.search_ranked("casa", filter, 10).unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].entry.source_id, expected);
        }
        assert!(
            store
                .search_ranked(
                    "casa",
                    SearchFilters {
                        headword_language: Some("en"),
                        ..Default::default()
                    },
                    10
                )
                .unwrap()
                .is_empty()
        );
    }


    #[test]
    fn typo_suggestions_are_bounded_source_scoped_and_accent_aware() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("a", "en")).unwrap();
        store.upsert_source(&source("b", "es")).unwrap();
        store
            .replace_source_entries(
                "a",
                &[
                    entry("a", "1", "casa", "house"),
                    entry("a", "2", "árbol", "tree"),
                    entry("a", "3", "casamiento", "wedding"),
                ],
            )
            .unwrap();
        store
            .replace_source_entries("b", &[entry("b", "1", "casas", "houses")])
            .unwrap();
        let english = SearchFilters {
            source_id: Some("a"),
            ..SearchFilters::default()
        };
        assert_eq!(store.suggest_words("casaa", english, 8).unwrap(), ["casa"]);
        assert_eq!(
            store.suggest_words("arboal", english, 8).unwrap(),
            ["árbol"]
        );
        assert!(store
            .suggest_words(
                "casaa",
                SearchFilters {
                    source_id: Some("missing"),
                    ..Default::default()
                },
                8
            )
            .unwrap()
            .is_empty());
        assert!(store.suggest_words("**", english, 8).unwrap().is_empty());
        assert!(store.suggest_words("casaa", english, 0).unwrap().is_empty());
    }

    #[test]
    fn edit_distance_threshold_and_unicode_codepoints() {
        assert_eq!(bounded_distance(&['c', 'a', 's', 'a'], "casas", 1), Some(1));
        assert_eq!(bounded_distance(&['c', 'a', 's', 'a'], "perro", 1), None);
        assert_eq!(fold_latin("NIÑO Árbol"), "nino arbol");
    }

    #[test]
    fn bounded_safe_query_and_empty_input() {
        let mut store = DictionaryStore::in_memory().unwrap();
        store.upsert_source(&source("a", "en")).unwrap();
        store
            .replace_source_entries("a", &[entry("a", "1", "casa", "house")])
            .unwrap();
        for query in ["", "\"", "***", "  "] {
            assert!(
                store
                    .search_ranked(query, SearchFilters::default(), 10)
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(
            store
                .search_ranked("casa", SearchFilters::default(), 0)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            store
                .search_ranked("casa", SearchFilters::default(), 200)
                .unwrap()
                .len(),
            1
        );
    }
}
