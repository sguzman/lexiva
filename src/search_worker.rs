//! Single long-lived search worker. SQLite searches never block egui.
//! The worker coalesces queued keystrokes and posts only tagged responses.
//! The app ignores stale tags and explicit source selection is mandatory.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
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
}

pub struct SearchUpdate {
    pub serial: u64,
    pub sources: Vec<DictionarySource>,
    pub hits: Vec<SearchHit>,
    pub error: Option<String>,
}

pub struct SearchWorker {
    requests: Sender<SearchRequest>,
    replies: Receiver<SearchUpdate>,
}

impl SearchWorker {
    pub fn spawn(path: PathBuf, context: &egui::Context) -> io::Result<Self> {
        let (request_tx, request_rx) = mpsc::channel::<SearchRequest>();
        let (reply_tx, reply_rx) = mpsc::channel::<SearchUpdate>();
        let context = context.clone();

        thread::Builder::new()
            .name("lexiva-search".to_owned())
            .spawn(move || {
                let connection = DictionaryStore::open(&path);
                while let Ok(mut request) = request_rx.recv() {
                    // Only the newest pending keystroke matters, but an
                    // already-running SQLite statement is bounded by its LIMIT.
                    while let Ok(newer) = request_rx.try_recv() {
                        request = newer;
                    }

                    let (sources, hits, error) = match &connection {
                        Ok(store) => match store.list_sources() {
                            Ok(sources) => {
                                let result = match &request.scope {
                                    SearchScope::Unselected => Ok(Vec::new()),
                                    _ if request.query.trim().is_empty() => Ok(Vec::new()),
                                    SearchScope::All => store.search_ranked(
                                        &request.query,
                                        SearchFilters::default(),
                                        RESULT_LIMIT,
                                    ),
                                    SearchScope::Source(id) => store.search_ranked(
                                        &request.query,
                                        SearchFilters {
                                            source_id: Some(id),
                                            ..SearchFilters::default()
                                        },
                                        RESULT_LIMIT,
                                    ),
                                };
                                match result {
                                    Ok(hits) => (sources, hits, None),
                                    Err(error) => (sources, Vec::new(), Some(error.to_string())),
                                }
                            }
                            Err(error) => (Vec::new(), Vec::new(), Some(error.to_string())),
                        },
                        Err(error) => (Vec::new(), Vec::new(), Some(error.to_string())),
                    };

                    if reply_tx
                        .send(SearchUpdate {
                            serial: request.serial,
                            sources,
                            hits,
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
        })
    }

    pub fn submit(&self, serial: u64, query: &str, scope: &SearchScope) -> Result<(), String> {
        self.requests
            .send(SearchRequest {
                serial,
                query: query.to_owned(),
                scope: scope.clone(),
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
    let parent = path.parent().ok_or_else(|| io::Error::other("invalid catalog path"))?;
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
    fn unselected_scope_never_means_search_every_source() {
        assert_eq!(SearchScope::default(), SearchScope::Unselected);
        assert_eq!(
            SearchScope::Unselected.label(&[]),
            "Choose a dictionary"
        );
        assert_eq!(SearchScope::All.label(&[]), "All indexed dictionaries");
    }
}
