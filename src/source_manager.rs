//! Explicit source discovery and import controls.
//! Scanning and indexing run on a background worker, never in egui's paint.
//! Original DICT and Wiktextract files are always opened read-only.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use eframe::egui;

use crate::dict::{self, DictFiles, DictReader};
use crate::model::DictionarySource;
use crate::storage::DictionaryStore;
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
    ImportDict(DiscoveredDict),
    ImportJsonl {
        path: PathBuf,
        edition: WiktionaryEdition,
        word_language: Option<String>,
    },
}

enum Event {
    Found(Vec<DiscoveredDict>),
    Progress(usize),
    Imported { source_id: String, count: usize },
    Error(String),
}

fn stable_source_id(prefix: &str, path: &Path, suffix: &str) -> String {
    // Stable, reproducible FNV-1a; the original path is also retained in
    // source metadata for identification. A different edition/filter is a
    // different logical Wiktextract index, not a destructive reimport.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.to_string_lossy().bytes().chain([0]).chain(suffix.bytes()) {
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

fn import_dict(
    store: &mut DictionaryStore,
    candidate: &DiscoveredDict,
    cancel: &AtomicBool,
    tx: &Sender<Event>,
) -> Result<usize, String> {
    let reader = DictReader::open(&candidate.source, &candidate.files)
        .map_err(|error| error.to_string())?;
    store
        .upsert_source(&candidate.source)
        .map_err(|error| error.to_string())?;
    store
        .import_stream(
            &candidate.source.id,
            reader,
            || cancel.load(Ordering::Acquire),
            |count| {
                let _ = tx.send(Event::Progress(count));
            },
        )
        .map_err(|error| error.to_string())
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
    let reader = WiktextractReader::open(path, &source, edition)
        .map_err(|error| error.to_string())?;
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
    Ok((id, count))
}

pub struct SourceManager {
    commands: Sender<Command>,
    events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub discovered: Vec<DiscoveredDict>,
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
                while let Ok(command) = incoming.recv() {
                    worker_cancel.store(false, Ordering::Release);
                    let result = match command {
                        Command::Scan(path) => match scan(&store, path.as_deref()) {
                            Ok(found) => {
                                let _ = outgoing.send(Event::Found(found));
                                Ok(None)
                            }
                            Err(error) => Err(error),
                        },
                        Command::ImportDict(candidate) => import_dict(
                            &mut store,
                            &candidate,
                            &worker_cancel,
                            &outgoing,
                        )
                        .map(|count| Some((candidate.source.id, count))),
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
                    context.request_repaint();
                }
            })?;
        Ok(Self {
            commands,
            events,
            cancel,
            discovered: Vec::new(),
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
    pub fn poll(&mut self) -> Option<String> {
        let mut imported = None;
        while let Ok(event) = self.events.try_recv() {
            match event {
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
                    imported = Some(source_id);
                }
                Event::Error(message) => {
                    self.busy = false;
                    self.status = format!("Failed: {message}");
                }
            }
        }
        imported
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
                            self.start(Command::ImportDict(candidate.clone()));
                        }
                    });
                    ui.small(candidate.files.index.display().to_string());
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
        let missing = std::env::temp_dir().join(format!(
            "lexiva-missing-folder-{}",
            std::process::id()
        ));
        // scan() also includes any system DICT files: it is therefore
        // tested by checking that it does not fail due to this missing folder.
        assert!(scan(&store, Some(&missing)).is_ok());
    }
}
