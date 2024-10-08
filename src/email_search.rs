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

pub struct EmailSearchTab {
    emails: Vec<String>,
    folder_path: Option<PathBuf>,
    email_list_path: Option<PathBuf>,
    search_in_progress: bool,
    progress: Arc<Mutex<(usize, usize)>>, // (processed, total)
    log_receiver: mpsc::Receiver<String>,
    log_sender: mpsc::Sender<String>,
    search_results: String, // Add this line
    results_receiver: Receiver<String>, // Add this line
}

impl EmailSearchTab {
    pub fn new() -> Self {
        let (log_sender, log_receiver) = mpsc::channel();
        let (results_sender, results_receiver) = mpsc::channel(); // Add this line
        Self {
            emails: Vec::new(),
            folder_path: None,
            email_list_path: None,
            search_in_progress: false,
            progress: Arc::new(Mutex::new((0, 0))),
            log_receiver,
            log_sender,
            search_results: String::new(), // Add this line
            results_receiver, // Add this line
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, processing_status: &mut String, tx: &Sender<String>) {
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
                let results_tx = mpsc::Sender::clone(&self.log_sender); // Use this instead of results_receiver.clone()

                self.search_in_progress = true;
                *processing_status = "Search in progress...".to_string();

                thread::spawn(move || {
                    let total_emails = emails.len();
                    *progress.lock().unwrap() = (0, total_emails);

                    for email in emails.iter() {
                        let result = search_email_main(email, &folder_path, log_tx.clone(), results_tx.clone());
                        match result {
                            Ok(message) => {
                                log_tx.send(message).unwrap();
                            },
                            Err(e) => {
                                log_tx.send(format!("Error: {}", e)).unwrap();
                            }
                        }
                        
                        let mut progress_guard = progress.lock().unwrap();
                        progress_guard.0 += 1;
                        log_tx.send(format!("Progress: {}/{}", progress_guard.0, progress_guard.1)).unwrap();
                    }

                    log_tx.send("Search completed.".to_string()).unwrap();
                });
            } else {
                *processing_status = "Please select a folder and email list first".to_string();
            }
        }

        // Display logs
        while let Ok(log) = self.log_receiver.try_recv() {
            ui.label(log);
        }

        if self.search_in_progress {
            let (processed, total) = *self.progress.lock().unwrap();
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

        // Add this block to update search_results
        while let Ok(result) = self.results_receiver.try_recv() {
            self.search_results.push_str(&result);
            self.search_results.push('\n');
        }

        // Display search results
        ui.separator();
        ui.label("Search Results:");
        ui.text_edit_multiline(&mut self.search_results);
    }

    fn load_emails(&mut self) {
        if let Some(path) = &self.email_list_path {
            let file = File::open(path).expect("Failed to open email list file");
            let reader = BufReader::new(file);
            self.emails = reader.lines().filter_map(Result::ok).collect();
        }
    }
}

fn search_email_in_folder(email: &str, folder_path: &Path, log_tx: Sender<String>, results_tx: Sender<String>) -> Result<(), Box<dyn std::error::Error>> {
    log_tx.send(format!("Searching in folder: {}", folder_path.display()))?;

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            search_email_in_folder(email, &path, log_tx.clone(), results_tx.clone())?;
        } else if path.is_file() && path.extension().map_or(false, |ext| ext == "csv") {
            log_tx.send(format!("Searching file: {}", path.display()))?;

            let file = File::open(&path)?;
            let transcoded = DecodeReaderBytesBuilder::new()
                .encoding(Some(WINDOWS_1252))
                .utf8_passthru(true)
                .build(file);
            let reader = BufReader::new(transcoded);

            for (row_index, line) in reader.lines().enumerate() {
                let line = line?;
                
                let fields: Vec<String> = line.split(',')
                    .map(|field| field.trim().to_string())
                    .collect();

                if fields.len() > 2 && (fields[0].to_lowercase() == email.to_lowercase() || 
                                        fields[2].to_lowercase() == email.to_lowercase()) {
                    let result = format!("Found email in file: {}, Row {}: {}", path.display(), row_index + 1, line);
                    results_tx.send(result.clone())?; // Send result immediately
                    log_tx.send(result)?;
                }
            }
        }
    }

    Ok(())
}

fn search_email_main(email: &str, folder_path: &Arc<PathBuf>, log_tx: mpsc::Sender<String>, results_tx: mpsc::Sender<String>) -> Result<String, Box<dyn std::error::Error>> {
    log_tx.send("Starting search...".to_string())?;
    
    search_email_in_folder(email, folder_path, log_tx.clone(), results_tx)?;
    
    Ok("Search completed successfully.".to_string())
}

