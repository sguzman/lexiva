//! Single long-lived search worker. SQLite searches never block egui.
//! The worker coalesces queued keystrokes and posts only tagged responses.
//! The app ignores stale tags and explicit source selection is mandatory.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use crate::model::DictionarySource;
use crate::storage::{DictionaryStore, SearchFilters, SearchHit};

const RESULT_LIMIT: usize = 60;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SearchScope {
    #[default]
    Unselected,
    All,
    Source(String),
}

impl SearchScope {
    /// Persist only deliberate scope choices, never guessing All for bad data.
    pub fn storage_value(&self) -> String {
        match self {
            Self::Unselected => "unselected".into(),
            Self::All => "all".into(),
            Self::Source(id) => format!("source:{id}"),
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "all" => Self::All,
            value if value.starts_with("source:") && value.len() > 7 => {
                Self::Source(value[7..].to_owned())
            }
            _ => Self::Unselected,
        }
    }

    pub fn label(&self, sources: &[DictionarySource]) -> String {
        match self {
            Self::Unselected => "Choose a dictionary".to_owned(),
            Self::All => "All indexed dictionaries".to_owned(),
            Self::Source(id) => sources
                .iter()
                .find(|source| source.id == *id)
                .map(|source| source.title.clone())
                .unwrap_or_else(|| format!("Unavailable: {id}")),
        }
    }
}

struct SearchRequest {
    serial: u64,
    query: String,
    scope: SearchScope,
    headword_language: Option<String>,
    definition_language: Option<String>,
}

pub struct SearchUpdate {
    pub serial: u64,
    pub sources: Vec<DictionarySource>,
    pub hits: Vec<SearchHit>,
    pub suggestions: Vec<String>,
    pub error: Option<String>,
}

pub struct SearchWorker {
    requests: Sender<SearchRequest>,
    replies: Receiver<SearchUpdate>,
    latest_serial: Arc<AtomicU64>,
    active_serial: Arc<AtomicU64>,
    interrupt: Arc<Mutex<Option<rusqlite::InterruptHandle>>>,
}

impl SearchWorker {
    pub fn spawn(path: PathBuf, context: &egui::Context) -> io::Result<Self> {
        let (request_tx, request_rx) = mpsc::channel::<SearchRequest>();
        let (reply_tx, reply_rx) = mpsc::channel::<SearchUpdate>();
        let context = context.clone();
        let latest_serial = Arc::new(AtomicU64::new(0));
        let active_serial = Arc::new(AtomicU64::new(0));
        let interrupt = Arc::new(Mutex::new(None::<rusqlite::InterruptHandle>));
        let worker_latest = latest_serial.clone();
        let worker_active = active_serial.clone();
        let worker_interrupt = interrupt.clone();

        thread::Builder::new()
            .name("lexiva-search".to_owned())
            .spawn(move || {
                let connection = DictionaryStore::open(&path);
                if let Ok(store) = &connection
                    && let Ok(mut guard) = worker_interrupt.lock()
                {
                    *guard = Some(store.interrupt_handle());
                }
                while let Ok(mut request) = request_rx.recv() {
                    // Only the newest pending keystroke matters, but an
                    // already-running SQLite statement is bounded by its LIMIT.
                    while let Ok(newer) = request_rx.try_recv() {
                        request = newer;
                    }

                    // Mark which query is executing so a newer user request
                    // can interrupt SQLite from the egui thread.
                    if worker_latest.load(Ordering::Acquire) != request.serial {
                        continue;
                    }
                    worker_active.store(request.serial, Ordering::Release);
                    let (sources, hits, suggestions, error) = match &connection {
                        Ok(store) => match store.list_enabled_sources() {
                            Ok(sources) => {
                                let filters = SearchFilters {
                                    source_id: match &request.scope {
                                        SearchScope::Source(id) => Some(id.as_str()),
                                        _ => None,
                                    },
                                    headword_language: request.headword_language.as_deref(),
                                    definition_language: request.definition_language.as_deref(),
                                };
                                let result = if request.scope == SearchScope::Unselected
                                    || request.query.trim().is_empty()
                                {
                                    Ok(Vec::new())
                                } else {
                                    store.search_ranked(&request.query, filters, RESULT_LIMIT)
                                };
                                match result {
                                    Ok(hits) => {
                                        let suggestions = if hits.is_empty()
                                            && !request.query.trim().is_empty()
                                            && request.scope != SearchScope::Unselected
                                        {
                                            store.suggest_words(&request.query, filters, 5)
                                        } else {
                                            Ok(Vec::new())
                                        };
                                        match suggestions {
                                            Ok(suggestions) => (sources, hits, suggestions, None),
                                            Err(error) => {
                                                (sources, hits, Vec::new(), Some(error.to_string()))
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        (sources, Vec::new(), Vec::new(), Some(error.to_string()))
                                    }
                                }
                            }
                            Err(error) => {
                                (Vec::new(), Vec::new(), Vec::new(), Some(error.to_string()))
                            }
                        },
                        Err(error) => (Vec::new(), Vec::new(), Vec::new(), Some(error.to_string())),
                    };
                    worker_active.store(0, Ordering::Release);
                    // A replaced query may have returned SQLITE_INTERRUPT;
                    // it must never appear as an error for the new query.
                    if worker_latest.load(Ordering::Acquire) != request.serial {
                        continue;
                    }

                    if reply_tx
                        .send(SearchUpdate {
                            serial: request.serial,
                            sources,
                            hits,
                            suggestions,
                            error,
                        })
                        .is_err()
                    {
                        return;
                    }
                    context.request_repaint();
                }
            })?;

        Ok(Self {
            requests: request_tx,
            replies: reply_rx,
            latest_serial,
            active_serial,
            interrupt,
        })
    }

    pub fn submit(
        &self,
        serial: u64,
        query: &str,
        scope: &SearchScope,
        headword_language: &str,
        definition_language: &str,
    ) -> Result<(), String> {
        // Advertise the new generation before interrupting the old SQLite
        // statement; the worker discards old results, including interruptions.
        self.latest_serial.store(serial, Ordering::Release);
        let active = self.active_serial.load(Ordering::Acquire);
        if active != 0
            && active != serial
            && let Ok(guard) = self.interrupt.lock()
            && let Some(handle) = guard.as_ref()
        {
            handle.interrupt();
        }
        self.requests
            .send(SearchRequest {
                serial,
                query: query.to_owned(),
                scope: scope.clone(),
                headword_language: optional_language(headword_language),
                definition_language: optional_language(definition_language),
            })
            .map_err(|_| "Dictionary search worker has stopped".to_owned())
    }

    pub fn poll(&self) -> Option<SearchUpdate> {
        let mut newest = None;
        while let Ok(update) = self.replies.try_recv() {
            newest = Some(update);
        }
        newest
    }
}

fn optional_language(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn catalog_path_for(xdg: Option<&Path>, home: Option<&Path>) -> io::Result<PathBuf> {
    let data_root = match xdg {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        _ => home
            .map(|path| path.join(".local").join("share"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not configured"))?,
    };
    Ok(data_root.join("lexiva").join("catalog.sqlite"))
}

/// XDG data location for the derived catalog; never touches package files.
pub fn catalog_path() -> io::Result<PathBuf> {
    let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path = catalog_path_for(xdg.as_deref(), home.as_deref())?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("invalid catalog path"))?;
    std::fs::create_dir_all(parent)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_absolute_xdg_data_path_and_home_fallback() {
        let home = Path::new("/home/test");
        assert_eq!(
            catalog_path_for(Some(Path::new("/data")), Some(home)).unwrap(),
            Path::new("/data/lexiva/catalog.sqlite")
        );
        assert_eq!(
            catalog_path_for(Some(Path::new("relative")), Some(home)).unwrap(),
            Path::new("/home/test/.local/share/lexiva/catalog.sqlite")
        );
        assert!(catalog_path_for(None, None).is_err());
    }

    #[test]
    fn worker_keeps_latest_query_and_source_identity() {
        use std::time::{Duration, Instant};

        use serde_json::Value;

        use crate::model::{DictionarySource, LexicalEntry, Sense, SourceFormat};

        let path = std::env::temp_dir().join(format!(
            "lexiva-search-worker-{}.sqlite",
            std::process::id()
        ));
        let mut store = DictionaryStore::open(&path).unwrap();
        let source = DictionarySource {
            id: "test".to_owned(),
            title: "Test dictionary".to_owned(),
            format: SourceFormat::Dict,
            origin: None,
            headword_language: Some("es".to_owned()),
            definition_language: Some("en".to_owned()),
            license: None,
            attribution: None,
            metadata: Value::Null,
        };
        store.upsert_source(&source).unwrap();
        store
            .replace_source_entries(
                "test",
                &[LexicalEntry {
                    source_id: "test".to_owned(),
                    source_entry_id: "1".to_owned(),
                    lemma: "casa".to_owned(),
                    headword_language: Some("es".to_owned()),
                    definition_language: Some("en".to_owned()),
                    part_of_speech: Some("noun".to_owned()),
                    senses: vec![Sense {
                        number: None,
                        definition_language: Some("en".to_owned()),
                        glosses: vec!["home".to_owned()],
                        labels: vec![],
                        examples: vec![],
                        children: vec![],
                    }],
                    pronunciations: vec![],
                    etymologies: vec![],
                    forms: vec![],
                    relations: vec![],
                    metadata: Value::Null,
                }],
            )
            .unwrap();
        drop(store);

        let context = egui::Context::default();
        let worker = SearchWorker::spawn(path.clone(), &context).unwrap();
        let scope = SearchScope::Source("test".to_owned());
        worker.submit(1, "missing", &scope, "es", "en").unwrap();
        worker.submit(2, "cas", &scope, "es", "en").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let update = loop {
            if let Some(update) = worker.poll()
                && update.serial == 2
            {
                break update;
            }
            assert!(Instant::now() < deadline, "search worker did not reply");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(update.hits.len(), 1);
        assert_eq!(update.hits[0].entry.lemma, "casa");
        assert_eq!(update.hits[0].source.id, "test");
        assert!(update.error.is_none());
        drop(worker);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn language_filter_inputs_are_independent_and_optional() {
        assert_eq!(optional_language(""), None);
        assert_eq!(optional_language("  es  "), Some("es".to_owned()));
        assert_eq!(optional_language("en"), Some("en".to_owned()));
    }

    #[test]
    fn saved_scope_round_trips_without_enabling_unselected_sources() {
        for scope in [
            SearchScope::Unselected,
            SearchScope::All,
            SearchScope::Source("dict:spa-eng:/usr/share/dictd".to_owned()),
        ] {
            assert_eq!(
                SearchScope::from_storage_value(&scope.storage_value()),
                scope
            );
        }
        for invalid in ["", "invalid", "source:", "ALL", "all indexed dictionaries"] {
            assert_eq!(
                SearchScope::from_storage_value(invalid),
                SearchScope::Unselected
            );
        }
    }

    #[test]
    fn unselected_scope_never_means_search_every_source() {
        assert_eq!(SearchScope::default(), SearchScope::Unselected);
        assert_eq!(SearchScope::Unselected.label(&[]), "Choose a dictionary");
        assert_eq!(SearchScope::All.label(&[]), "All indexed dictionaries");
    }
}
