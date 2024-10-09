use eframe::egui;
use std::path::{PathBuf, Path};
use std::sync::{Arc, Mutex, mpsc::{Sender, Receiver, channel}};
use std::thread;
use egui::RichText;
use rfd::FileDialog;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use encoding_rs_io::DecodeReaderBytesBuilder;
use encoding_rs::WINDOWS_1252;
use std::sync::mpsc;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashSet;

pub struct EmailSearchTab {
    emails: Vec<String>,
    folder_path: Option<PathBuf>,
    email_list_path: Option<PathBuf>,
    search_in_progress: bool,
    progress: Arc<(AtomicUsize, AtomicUsize)>, // (processed, total)
    log_receiver: mpsc::Receiver<String>,
    log_sender: mpsc::Sender<String>,
    // Removed: search_results: String,
    results_file_path: Option<PathBuf>,
    found_emails: Arc<Mutex<HashSet<String>>>,
}

impl EmailSearchTab {
    pub fn new() -> Self {
        let (log_sender, log_receiver) = channel();
        Self {
            emails: Vec::new(),
            folder_path: None,
            email_list_path: None,
            search_in_progress: false,
            progress: Arc::new((AtomicUsize::new(0), AtomicUsize::new(0))),
            log_receiver,
            log_sender,
            // Removed: search_results: String::new(),
            results_file_path: None,
            found_emails: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, processing_status: &mut String, tx: &Sender<String>) {
        // UI elements and button handling...
        ui.horizontal(|ui| {
            if ui.button("Select Email List").clicked() {
                if let Some(file_path) = FileDialog::new().add_filter("Text file", &["txt"]).pick_file() {
                    self.email_list_path = Some(file_path);
                    self.load_emails();
                }
            }
            if let Some(path) = &self.email_list_path {
                ui.label(format!("Selected email list: {}", path.display()));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Select Folder").clicked() {
                if let Some(folder) = FileDialog::new().pick_folder() {
                    self.folder_path = Some(folder);
                }
            }
            if let Some(path) = &self.folder_path {
                ui.label(format!("Selected folder: {}", path.display()));
            }
        });
        if ui.button("Search").clicked() && !self.search_in_progress {
            if let (Some(folder), Some(_)) = (&self.folder_path, &self.email_list_path) {
                let emails = Arc::new(self.emails.clone());
                let folder_path = Arc::new(folder.clone());
                let progress = self.progress.clone();
                let log_tx = tx.clone();
                
                // Create results file
                let results_file_path = folder.join("search_results.csv");
                self.results_file_path = Some(results_file_path.clone());
                
                let found_emails = self.found_emails.clone();
                
                match create_results_file(&results_file_path) {
                    Ok(file) => {
                        let results_file = Arc::new(Mutex::new(file));

                        self.search_in_progress = true;
                        *processing_status = "Search in progress...".to_string();

                        thread::spawn(move || {
                            let total_emails = emails.len();
                            progress.1.store(total_emails, Ordering::Relaxed);

                            emails.par_iter().enumerate().for_each(|(index, email)| {
                                if let Err(e) = search_email_main(email, &folder_path, log_tx.clone(), results_file.clone(), progress.clone(), index, found_emails.clone()) {
                                    log_tx.send(format!("Error searching email {}: {}", email, e)).unwrap();
                                }
                            });

                            log_tx.send("Search completed.".to_string()).unwrap();
                        });
                    },
                    Err(e) => {
                        *processing_status = format!("Error creating results file: {}", e);
                    }
                }
            } else {
                *processing_status = "Please select a folder and email list first".to_string();
            }
        }

        // Display logs
        while let Ok(log) = self.log_receiver.try_recv() {
            ui.label(log);
        }

        if self.search_in_progress {
            let processed = self.progress.0.load(Ordering::Relaxed);
            let total = self.progress.1.load(Ordering::Relaxed);
            ui.add(egui::ProgressBar::new(processed as f32 / total as f32)
                .text(format!("Processed: {}/{}", processed, total)));
        }

        // Display processing status
        if !processing_status.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Status:").strong());
                ui.label(&*processing_status);
            });
        }
        // Display results file path
        if let Some(path) = &self.results_file_path {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Results file:").strong());
                ui.label(path.display().to_string());
            });
        }
    }


    fn load_emails(&mut self) {
        if let Some(path) = &self.email_list_path {
            let file = File::open(path).expect("Failed to open email list file");
            let reader = BufReader::new(file);
            self.emails = reader.lines().filter_map(Result::ok).collect();
        }
    }
}

fn create_results_file(path: &Path) -> Result<File, std::io::Error> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    Ok(file)
}

fn search_email_in_folder(
    email: &str, 
    folder_path: &Path, 
    log_tx: Sender<String>, 
    results_file: Arc<Mutex<File>>,
    found_emails: Arc<Mutex<HashSet<String>>>
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if email has already been found
    if found_emails.lock().unwrap().contains(email) {
        return Ok(());
    }

    log_tx.send(format!("Searching in folder: {}", folder_path.display()))?;

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            search_email_in_folder(email, &path, log_tx.clone(), results_file.clone(), found_emails.clone())?;
        } else if path.is_file() && path.extension().map_or(false, |ext| ext == "csv") {
            log_tx.send(format!("Searching file: {}", path.display()))?;

            let file = File::open(&path)?;
            let transcoded = DecodeReaderBytesBuilder::new()
                .encoding(Some(WINDOWS_1252))
                .utf8_passthru(true)
                .build(file);
            let reader = BufReader::new(transcoded);

            let mut email_found = false;
            reader.lines()
                .enumerate()
                .filter_map(|(row_index, line_result)| line_result.ok().map(|line| (row_index, line)))
                .filter(|(_, line)| {
                    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
                    fields.len() > 2 && (fields[0].eq_ignore_ascii_case(email) || fields[2].eq_ignore_ascii_case(email))
                })
                .for_each(|(row_index, line)| {
                    if !email_found {
                        let result = format!("{},{}\n", row_index + 1, line);
                        if let Err(e) = results_file.lock().unwrap().write_all(result.as_bytes()) {
                            log_tx.send(format!("Error writing result: {}", e)).unwrap();
                        } else {
                            log_tx.send(format!("Match found: {}", result)).unwrap();
                            email_found = true;
                            found_emails.lock().unwrap().insert(email.to_string());
                        }
                    }
                });

            if email_found {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn search_email_main(
    email: &str, 
    folder_path: &Arc<PathBuf>, 
    log_tx: mpsc::Sender<String>, 
    results_file: Arc<Mutex<File>>, 
    progress: Arc<(AtomicUsize, AtomicUsize)>, 
    index: usize,
    found_emails: Arc<Mutex<HashSet<String>>>
) -> Result<String, Box<dyn std::error::Error>> {
    log_tx.send("Starting search...".to_string())?;
    
    search_email_in_folder(email, folder_path, log_tx.clone(), results_file, found_emails)?;
    
    // Update progress
    progress.0.fetch_add(1, Ordering::Relaxed);
    
    Ok("Search completed successfully.".to_string())
}