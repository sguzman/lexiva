//! Explicit source discovery and import controls.
//! Scanning and indexing run on a background worker, never in egui's paint.
//! Original DICT and Wiktextract files are always opened read-only.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui;

use crate::dict::{self, DictFiles, DictReader};
use crate::model::DictionarySource;
use crate::storage::{DictionaryStore, SourceSettings};
use crate::wiktextract::{self, WiktextractReader, WiktionaryEdition};

const MAX_DISCOVERED: usize = 256;

#[derive(Clone, Debug)]
pub struct DiscoveredDict {
    pub source: DictionarySource,
    pub files: DictFiles,
    pub indexed_entries: i64,
}

enum Command {
    Scan(Option<PathBuf>),
    Refresh,
    Reindex(String),
    ImportDict(Box<DiscoveredDict>),
    Remove(String),
    SetEnabled(String, bool),
    MovePriority(String, isize),
    ImportJsonl {
        path: PathBuf,
        edition: WiktionaryEdition,
        word_language: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub enum ManagerChange {
    Imported(String),
    Removed(String),
    Preferences(String),
}

#[derive(Clone, Debug)]
struct IndexedSource {
    settings: SourceSettings,
    count: i64,
    health: SourceHealth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceHealth {
    UpToDate,
    Changed,
    Missing,
    Unverified,
}

impl SourceHealth {
    const fn label(self) -> &'static str {
        match self {
            Self::UpToDate => "Up to date",
            Self::Changed => "Source changed — reindex",
            Self::Missing => "Source file missing — cached index retained",
            Self::Unverified => "Not yet verified — reindex",
        }
    }
}

/// A bounded signature: size + high-resolution modification time + sampled
/// first/last 8 KiB of both DICT files (or the selected JSONL). This detects
/// typical package changes without scanning a multi-gigabyte corpus.
fn file_signature(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let info = file.metadata()?;
    let modified = info
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hash = 0xcbf29ce484222325_u64;
    for position in [0, info.len().saturating_sub(8192)] {
        file.seek(SeekFrom::Start(position))?;
        let mut content = Vec::with_capacity(8192);
        file.by_ref().take(8192).read_to_end(&mut content)?;
        for byte in content {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!(
        "{}:{}:{hash:016x}",
        info.len(),
        modified.as_nanos()
    ))
}

fn source_signature(source: &DictionarySource) -> io::Result<String> {
    let origin = source
        .origin
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing source path"))?;
    let first = file_signature(Path::new(origin))?;
    if source.format == crate::model::SourceFormat::Dict {
        let data = source.metadata["dict"]["data_path"]
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing DICT data path"))?;
        Ok(format!("{first}|{}", file_signature(Path::new(data))?))
    } else {
        Ok(first)
    }
}

fn source_health(source: &DictionarySource) -> SourceHealth {
    match source_signature(source) {
        Err(_) => SourceHealth::Missing,
        Ok(actual) => match source.metadata["import_signature"].as_str() {
            None => SourceHealth::Unverified,
            Some(stored) if stored == actual => SourceHealth::UpToDate,
            Some(_) => SourceHealth::Changed,
        },
    }
}

fn record_signature(
    store: &mut DictionaryStore,
    source_id: &str,
    signature: String,
) -> Result<(), String> {
    let mut source = store
        .get_source(source_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "source disappeared during import".to_owned())?;
    if !source.metadata.is_object() {
        source.metadata = json!({});
    }
    source.metadata["import_signature"] = json!(signature);
    store
        .upsert_source(&source)
        .map_err(|error| error.to_string())
}

enum Event {
    Found(Vec<DiscoveredDict>),
    Catalog(Vec<IndexedSource>),
    Removed(String),
    Preferences(String),
    Checked,
    Progress(usize),
    Imported { source_id: String, count: usize },
    Error(String),
}

fn stable_source_id(prefix: &str, path: &Path, suffix: &str) -> String {
    // Stable, reproducible FNV-1a; the original path is also retained in
    // source metadata for identification. A different edition/filter is a
    // different logical Wiktextract index, not a destructive reimport.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path
        .to_string_lossy()
        .bytes()
        .chain([0])
        .chain(suffix.bytes())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}-{hash:016x}")
}

fn scan(store: &DictionaryStore, directory: Option<&Path>) -> Result<Vec<DiscoveredDict>, String> {
    let mut locations = dict::system_dictd_directories().to_vec();
    if let Some(directory) = directory {
        locations.push(directory.to_owned());
    }
    let paths = dict::discover_in(&locations).map_err(|error| error.to_string())?;
    if paths.len() > MAX_DISCOVERED {
        return Err(format!(
            "Found {} dictionaries, exceeding the safety limit of {MAX_DISCOVERED}",
            paths.len()
        ));
    }
    paths
        .into_iter()
        .map(|files| {
            let id = stable_source_id("dict", &files.index, "");
            let source = dict::metadata::source_from_files(&files, &id)
                .map_err(|error| format!("{}: {error}", files.index.display()))?;
            let indexed_entries = store
                .count_entries(&id)
                .map_err(|error| error.to_string())?;
            Ok(DiscoveredDict {
                source,
                files,
                indexed_entries,
            })
        })
        .collect()
}

fn indexed_catalog(store: &DictionaryStore) -> Result<Vec<IndexedSource>, String> {
    store
        .list_source_settings()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|settings| {
            let count = store
                .count_entries(&settings.source.id)
                .map_err(|error| error.to_string())?;
            let health = source_health(&settings.source);
            Ok(IndexedSource {
                settings,
                count,
                health,
            })
        })
        .collect()
}

fn import_dict(
    store: &mut DictionaryStore,
    candidate: &DiscoveredDict,
    cancel: &AtomicBool,
    tx: &Sender<Event>,
) -> Result<usize, String> {
    let signature = source_signature(&candidate.source).map_err(|error| error.to_string())?;
    let reader =
        DictReader::open(&candidate.source, &candidate.files).map_err(|error| error.to_string())?;
    store
        .upsert_source(&candidate.source)
        .map_err(|error| error.to_string())?;
    let count = store
        .import_stream(
            &candidate.source.id,
            reader,
            || cancel.load(Ordering::Acquire),
            |count| {
                let _ = tx.send(Event::Progress(count));
            },
        )
        .map_err(|error| error.to_string())?;
    record_signature(store, &candidate.source.id, signature)?;
    Ok(count)
}

fn import_jsonl(
    store: &mut DictionaryStore,
    path: &Path,
    edition: WiktionaryEdition,
    word_language: Option<&str>,
    cancel: &AtomicBool,
    tx: &Sender<Event>,
) -> Result<(String, usize), String> {
    let suffix = format!("{}:{}", edition.code(), word_language.unwrap_or("*"));
    let id = stable_source_id("wiktextract", path, &suffix);
    let title = path
        .file_name()
        .map(|file| file.to_string_lossy().into_owned())
        .ok_or_else(|| "JSONL file has no filename".to_owned())?;
    let source = wiktextract::source_from_jsonl(
        path,
        &id,
        &format!("{title} ({suffix})"),
        edition,
        word_language,
    )
    .map_err(|error| error.to_string())?;
    let signature = source_signature(&source).map_err(|error| error.to_string())?;
    let reader =
        WiktextractReader::open(path, &source, edition).map_err(|error| error.to_string())?;
    store
        .upsert_source(&source)
        .map_err(|error| error.to_string())?;
    let count = store
        .import_stream(
            &id,
            reader,
            || cancel.load(Ordering::Acquire),
            |count| {
                let _ = tx.send(Event::Progress(count));
            },
        )
        .map_err(|error| error.to_string())?;
    record_signature(store, &id, signature)?;
    Ok((id, count))
}

fn reindex(
    store: &mut DictionaryStore,
    source_id: &str,
    cancel: &AtomicBool,
    tx: &Sender<Event>,
) -> Result<usize, String> {
    let source = store
        .get_source(source_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "dictionary is not indexed".to_owned())?;
    match source.format {
        crate::model::SourceFormat::Dict => {
            let path = source
                .origin
                .as_ref()
                .ok_or_else(|| "missing DICT index path".to_owned())?;
            let data = source.metadata["dict"]["data_path"]
                .as_str()
                .ok_or_else(|| "missing DICT data path".to_owned())?;
            let files = DictFiles {
                index: PathBuf::from(path),
                data: PathBuf::from(data),
                compressed: source.metadata["dict"]["compression"] == "dictzip",
            };
            import_dict(
                store,
                &DiscoveredDict {
                    source,
                    files,
                    indexed_entries: 0,
                },
                cancel,
                tx,
            )
        }
        crate::model::SourceFormat::Wiktextract => {
            let path = source
                .origin
                .as_ref()
                .ok_or_else(|| "missing JSONL path".to_owned())?
                .clone();
            let edition = match source.definition_language.as_deref() {
                Some("en") => WiktionaryEdition::English,
                Some("es") => WiktionaryEdition::Spanish,
                _ => return Err("unsupported Wiktionary edition".into()),
            };
            let (id, count) = import_jsonl(
                store,
                Path::new(&path),
                edition,
                source.headword_language.as_deref(),
                cancel,
                tx,
            )?;
            if id != source_id {
                return Err("source ID changed; existing index was not replaced".into());
            }
            Ok(count)
        }
        _ => Err("reindexing this dictionary format is not supported yet".into()),
    }
}

pub struct SourceManager {
    commands: Sender<Command>,
    events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub discovered: Vec<DiscoveredDict>,
    indexed: Vec<IndexedSource>,
    directory: String,
    jsonl_path: String,
    edition: WiktionaryEdition,
    word_language: String,
    busy: bool,
    progress: usize,
    status: String,
}

impl SourceManager {
    pub fn spawn(path: PathBuf, context: &egui::Context) -> io::Result<Self> {
        let (commands, incoming) = mpsc::channel::<Command>();
        let (outgoing, events) = mpsc::channel::<Event>();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let context = context.clone();
        thread::Builder::new()
            .name("lexiva-import".into())
            .spawn(move || {
                let mut store = match DictionaryStore::open(&path) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = outgoing.send(Event::Error(error.to_string()));
                        context.request_repaint();
                        return;
                    }
                };
                if let Ok(indexed) = indexed_catalog(&store) {
                    let _ = outgoing.send(Event::Catalog(indexed));
                    context.request_repaint();
                }
                while let Ok(command) = incoming.recv() {
                    let result = match command {
                        Command::Refresh => {
                            let _ = outgoing.send(Event::Checked);
                            Ok(None)
                        }
                        Command::Reindex(id) => reindex(&mut store, &id, &worker_cancel, &outgoing)
                            .map(|count| Some((id, count))),
                        Command::Scan(path) => match scan(&store, path.as_deref()) {
                            Ok(found) => {
                                let _ = outgoing.send(Event::Found(found));
                                Ok(None)
                            }
                            Err(error) => Err(error),
                        },
                        Command::ImportDict(candidate) => {
                            import_dict(&mut store, &candidate, &worker_cancel, &outgoing)
                                .map(|count| Some((candidate.source.id, count)))
                        }
                        Command::SetEnabled(id, enabled) => store
                            .set_source_enabled(&id, enabled)
                            .map_err(|error| error.to_string())
                            .map(|()| {
                                let _ = outgoing.send(Event::Preferences(id));
                                None
                            }),
                        Command::MovePriority(id, direction) => store
                            .move_source(&id, direction)
                            .map_err(|error| error.to_string())
                            .map(|moved| {
                                if moved {
                                    let _ = outgoing.send(Event::Preferences(id));
                                }
                                None
                            }),
                        Command::Remove(id) => store
                            .remove_source(&id)
                            .map_err(|error| error.to_string())
                            .map(|removed| {
                                if removed {
                                    let _ = outgoing.send(Event::Removed(id));
                                }
                                None
                            }),
                        Command::ImportJsonl {
                            path,
                            edition,
                            word_language,
                        } => import_jsonl(
                            &mut store,
                            &path,
                            edition,
                            word_language.as_deref(),
                            &worker_cancel,
                            &outgoing,
                        )
                        .map(Some),
                    };
                    match result {
                        Ok(Some((source_id, count))) => {
                            let _ = outgoing.send(Event::Imported { source_id, count });
                        }
                        Ok(None) => {}
                        Err(message) => {
                            let _ = outgoing.send(Event::Error(message));
                        }
                    }
                    if let Ok(indexed) = indexed_catalog(&store) {
                        let _ = outgoing.send(Event::Catalog(indexed));
                    }
                    context.request_repaint();
                }
            })?;
        Ok(Self {
            commands,
            events,
            cancel,
            discovered: Vec::new(),
            indexed: Vec::new(),
            directory: String::new(),
            jsonl_path: String::new(),
            edition: WiktionaryEdition::English,
            word_language: "es".into(),
            busy: false,
            progress: 0,
            status: "Choose Scan to look for installed DICT files.".into(),
        })
    }

    fn start(&mut self, command: Command) {
        if self.busy {
            return;
        }
        self.cancel.store(false, Ordering::Release);
        match self.commands.send(command) {
            Ok(()) => {
                self.busy = true;
                self.progress = 0;
                self.status = "Working…".into();
            }
            Err(_) => {
                self.status = "Import worker is unavailable.".into();
            }
        }
    }

    /// Returns a new indexed source ID so the search worker can refresh.
    pub fn poll(&mut self) -> Option<ManagerChange> {
        let mut changed = None;
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Checked => {
                    self.busy = false;
                    self.status = "Source files checked. Indexed entries are preserved.".into();
                }
                Event::Catalog(indexed) => {
                    self.indexed = indexed;
                }
                Event::Preferences(id) => {
                    self.busy = false;
                    self.status = "Dictionary preferences saved.".into();
                    changed = Some(ManagerChange::Preferences(id));
                }
                Event::Removed(id) => {
                    self.busy = false;
                    self.status = "Removed local index; dictionary files are unchanged.".into();
                    for candidate in &mut self.discovered {
                        if candidate.source.id == id {
                            candidate.indexed_entries = 0;
                        }
                    }
                    changed = Some(ManagerChange::Removed(id));
                }
                Event::Found(found) => {
                    self.discovered = found;
                    self.busy = false;
                    self.status = format!("Discovered {} DICT file pairs.", self.discovered.len());
                }
                Event::Progress(count) => {
                    self.progress = count;
                }
                Event::Imported { source_id, count } => {
                    self.busy = false;
                    self.status = format!("Indexed {count} entries. Source ready.");
                    for candidate in &mut self.discovered {
                        if candidate.source.id == source_id {
                            candidate.indexed_entries = count as i64;
                        }
                    }
                    changed = Some(ManagerChange::Imported(source_id));
                }
                Event::Error(message) => {
                    self.busy = false;
                    self.status = format!("Failed: {message}");
                }
            }
        }
        changed
    }

    pub fn is_enabled(&self, source_id: &str) -> bool {
        self.indexed
            .iter()
            .any(|item| item.settings.source.id == source_id && item.settings.enabled)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Dictionary sources — discover and import")
            .id_salt("lexiva_dictionary_manager")
            .show(ui, |ui| {
                ui.label("Scan installed DICT directories, optionally including another directory.");
                ui.add(
                    egui::TextEdit::singleline(&mut self.directory)
                        .hint_text("Optional additional DICT directory")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.busy, egui::Button::new("Scan DICT files")).clicked() {
                        let extra = self.directory.trim();
                        self.start(Command::Scan(if extra.is_empty() {
                            None
                        } else {
                            Some(PathBuf::from(extra))
                        }));
                    }
                    if self.busy && ui.button("Cancel import").clicked() {
                        self.cancel.store(true, Ordering::Release);
                        self.status = "Cancellation requested…".into();
                    }
                });
                for candidate in self.discovered.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(&candidate.source.title);
                        ui.weak(format!(
                            "{} → {} · {} indexed",
                            candidate.source.headword_language.as_deref().unwrap_or("?"),
                            candidate.source.definition_language.as_deref().unwrap_or("?"),
                            candidate.indexed_entries
                        ));
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Import / reindex"))
                            .clicked()
                        {
                            self.start(Command::ImportDict(Box::new(candidate.clone())));
                        }
                    });
                    ui.small(candidate.files.index.display().to_string());
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong("Indexed dictionary sources");
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Check source files"))
                        .clicked()
                    {
                        self.start(Command::Refresh);
                    }
                });
                for (index, item) in self.indexed.clone().into_iter().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        let mut enabled = item.settings.enabled;
                        if ui
                            .add_enabled(
                                !self.busy,
                                egui::Checkbox::new(&mut enabled, "Enabled"),
                            )
                            .changed()
                        {
                            self.start(Command::SetEnabled(
                                item.settings.source.id.clone(),
                                enabled,
                            ));
                        }
                        ui.label(format!(
                            "{} · {} entries",
                            item.settings.source.title, item.count
                        ));
                        ui.weak(item.health.label());
                        if ui
                            .add_enabled(
                                !self.busy && item.health != SourceHealth::Missing,
                                egui::Button::new("Reindex"),
                            )
                            .clicked()
                        {
                            self.start(Command::Reindex(item.settings.source.id.clone()));
                        }
                        if ui
                            .add_enabled(!self.busy && index > 0, egui::Button::new("↑"))
                            .clicked()
                        {
                            self.start(Command::MovePriority(
                                item.settings.source.id.clone(),
                                -1,
                            ));
                        }
                        if ui
                            .add_enabled(
                                !self.busy && index + 1 < self.indexed.len(),
                                egui::Button::new("↓"),
                            )
                            .clicked()
                        {
                            self.start(Command::MovePriority(
                                item.settings.source.id.clone(),
                                1,
                            ));
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Remove local index"))
                            .clicked()
                        {
                            self.start(Command::Remove(item.settings.source.id.clone()));
                        }
                    });
                }
                ui.separator();
                ui.label("Import Wiktextract JSONL / JSONL.gz");
                ui.add(
                    egui::TextEdit::singleline(&mut self.jsonl_path)
                        .hint_text("Absolute path to local .jsonl or .jsonl.gz file")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.label("Wiktionary edition (definitions)");
                    egui::ComboBox::from_id_salt("wiktionary_edition")
                        .selected_text(match self.edition {
                            WiktionaryEdition::English => "English",
                            WiktionaryEdition::Spanish => "Spanish",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edition,
                                WiktionaryEdition::English,
                                "English",
                            );
                            ui.selectable_value(
                                &mut self.edition,
                                WiktionaryEdition::Spanish,
                                "Spanish",
                            );
                        });
                    ui.label("Headword filter");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.word_language)
                            .desired_width(65.0),
                    );
                });
                if ui
                    .add_enabled(
                        !self.busy && !self.jsonl_path.trim().is_empty(),
                        egui::Button::new("Import selected JSONL file"),
                    )
                    .clicked()
                {
                    let language = self.word_language.trim();
                    self.start(Command::ImportJsonl {
                        path: PathBuf::from(self.jsonl_path.trim()),
                        edition: self.edition,
                        word_language: if language.is_empty() {
                            None
                        } else {
                            Some(language.to_owned())
                        },
                    });
                }
                if self.busy {
                    ui.spinner();
                    ui.label(format!("{} entries staged", self.progress));
                }
                ui.label(&self.status);
                ui.weak("Source files are read-only. Imports publish atomically; cancel preserves the previous index.");
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_dict_and_jsonl_imports_publish_to_catalog_without_editing_files() {
        use std::fs;
        use std::sync::atomic::AtomicUsize;

        use crate::storage::SearchFilters;

        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "lexiva-manager-test-{}-{index}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let index_path = root.join("freedict-spa-eng.index");
        let data_path = root.join("freedict-spa-eng.dict");
        let jsonl_path = root.join("raw.jsonl");
        let original_index = b"casa\tA\tF\n";
        let original_data = b"home\n";
        let original_jsonl =
            b"{\"word\":\"hogar\",\"lang_code\":\"es\",\"pos\":\"noun\",\"senses\":[{\"glosses\":[\"A home\"]}]}\n";
        fs::write(&index_path, original_index).unwrap();
        fs::write(&data_path, original_data).unwrap();
        fs::write(&jsonl_path, original_jsonl).unwrap();

        let mut store = DictionaryStore::open(&root.join("catalog.sqlite")).unwrap();
        let files = DictFiles {
            index: index_path.clone(),
            data: data_path.clone(),
            compressed: false,
        };
        let id = stable_source_id("dict", &index_path, "");
        let source = dict::metadata::source_from_files(&files, &id).unwrap();
        let candidate = DiscoveredDict {
            source,
            files,
            indexed_entries: 0,
        };
        let (tx, rx) = mpsc::channel();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            import_dict(&mut store, &candidate, &cancel, &tx).unwrap(),
            1
        );
        assert_eq!(store.count_entries(&id).unwrap(), 1);
        assert_eq!(
            store.get_entry(&id, "index-line:1").unwrap().unwrap().lemma,
            "casa"
        );
        assert_eq!(
            store
                .search_ranked(
                    "cas",
                    SearchFilters {
                        source_id: Some(&id),
                        ..SearchFilters::default()
                    },
                    10
                )
                .unwrap()
                .len(),
            1
        );
        assert!(matches!(rx.try_recv(), Ok(Event::Progress(1))));

        let (json_id, count) = import_jsonl(
            &mut store,
            &jsonl_path,
            WiktionaryEdition::English,
            Some("es"),
            &cancel,
            &tx,
        )
        .unwrap();
        assert_eq!(count, 1);
        let json_entry = store.get_entry(&json_id, "jsonl-line:1").unwrap().unwrap();
        assert_eq!(json_entry.lemma, "hogar");
        assert_eq!(json_entry.definition_language.as_deref(), Some("en"));

        // An immediately cancelled reindex must leave the last published
        // record available, with no staging rows leaked.
        cancel.store(true, Ordering::Release);
        assert!(import_dict(&mut store, &candidate, &cancel, &tx).is_err());
        assert_eq!(store.count_entries(&id).unwrap(), 1);
        assert_eq!(store.staged_count(&id).unwrap(), 0);

        // Same-size edits are caught by sampled content fingerprints,
        // even if a package preserves the compressed file size.
        cancel.store(false, Ordering::Release);
        let indexed_source = store.get_source(&id).unwrap().unwrap();
        assert_eq!(source_health(&indexed_source), SourceHealth::UpToDate);
        fs::write(&data_path, b"nest\n").unwrap();
        assert_eq!(source_health(&indexed_source), SourceHealth::Changed);
        assert_eq!(reindex(&mut store, &id, &cancel, &tx).unwrap(), 1);
        let updated = store.get_entry(&id, "index-line:1").unwrap().unwrap();
        assert_eq!(updated.senses[0].glosses[0], "nest\n");
        assert_eq!(
            source_health(&store.get_source(&id).unwrap().unwrap()),
            SourceHealth::UpToDate
        );
        fs::remove_file(&data_path).unwrap();
        assert_eq!(
            source_health(&store.get_source(&id).unwrap().unwrap()),
            SourceHealth::Missing
        );
        assert_eq!(store.count_entries(&id).unwrap(), 1);
        assert!(reindex(&mut store, &id, &cancel, &tx).is_err());
        assert_eq!(store.count_entries(&id).unwrap(), 1);
        fs::write(&data_path, original_data).unwrap();

        cancel.store(false, Ordering::Release);
        assert!(store.remove_source(&id).unwrap());
        assert_eq!(store.count_entries(&id).unwrap(), 0);
        assert!(
            store
                .search_ranked("casa", SearchFilters::default(), 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.count_entries(&json_id).unwrap(), 1);

        assert_eq!(fs::read(&index_path).unwrap(), original_index);
        assert_eq!(fs::read(&data_path).unwrap(), original_data);
        assert_eq!(fs::read(&jsonl_path).unwrap(), original_jsonl);
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stable_source_ids_separate_paths_edition_and_filter() {
        let first = stable_source_id("dict", Path::new("/usr/share/dictd/a.index"), "");
        assert_eq!(
            first,
            stable_source_id("dict", Path::new("/usr/share/dictd/a.index"), "")
        );
        assert_ne!(
            first,
            stable_source_id("dict", Path::new("/usr/share/dictd/b.index"), "")
        );
        assert_ne!(
            stable_source_id("wiktextract", Path::new("/tmp/raw.jsonl"), "en:es"),
            stable_source_id("wiktextract", Path::new("/tmp/raw.jsonl"), "es:es")
        );
    }

    #[test]
    fn missing_dictionary_folder_is_empty_not_error() {
        let store = DictionaryStore::in_memory().unwrap();
        let missing =
            std::env::temp_dir().join(format!("lexiva-missing-folder-{}", std::process::id()));
        // scan() also includes any system DICT files: it is therefore
        // tested by checking that it does not fail due to this missing folder.
        assert!(scan(&store, Some(&missing)).is_ok());
    }
}
