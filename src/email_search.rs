use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread;
use egui::RichText;
use rfd::FileDialog;
use std::fs::File;
use std::io::{BufRead, BufReader};
use encoding_rs_io::DecodeReaderBytesBuilder;
use encoding_rs::WINDOWS_1252;
use std::path::Path;  // Add this import at the top of the file

pub struct EmailSearchTab {
    email: String,
    folder_path: Option<PathBuf>,
}

impl EmailSearchTab {
    pub fn new() -> Self {
        Self {
            email: String::from(""),
            folder_path: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, processing_status: &mut String, tx: &Sender<String>) {
        ui.horizontal(|ui| {
            ui.label("Email:");
            ui.text_edit_singleline(&mut self.email);
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

        if ui.button("Search").clicked() {
            if let Some(folder) = &self.folder_path {
                let email = self.email.clone();
                let folder_path = folder.clone();
                let tx_clone = tx.clone();

                thread::spawn(move || {
                    let (log_tx, log_rx) = channel();
                    let tx_for_search = tx_clone.clone();
                    
                    thread::spawn(move || {
                        match search_email_main(&email, &folder_path, log_tx) {
                            Ok(result) => {
                                tx_for_search.send(result).unwrap();
                            }
                            Err(e) => {
                                tx_for_search.send(format!("Error: {}", e)).unwrap();
                            }
                        }
                    });

                    // Print logs in the main thread
                    for log in log_rx {
                        println!("{}", log);
                        tx_clone.send(log).unwrap();
                    }
                });
            } else {
                *processing_status = "Please select a folder first".to_string();
            }
        }

        // Display processing status
        if !processing_status.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Status:").strong());
                ui.label(&*processing_status);
            });
        }
    }
}

fn search_email_in_folder(email: &str, folder_path: &Path, log_tx: Sender<String>) -> Result<Option<String>, Box<dyn std::error::Error>> {
    log_tx.send(format!("Searching in folder: {}", folder_path.display()))?;
    
    for entry in std::fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            // Recursively search in subfolders
            if let Some(result) = search_email_in_folder(email, &path, log_tx.clone())? {
                return Ok(Some(result));
            }
        } else if path.is_file() && path.extension().map_or(false, |ext| ext == "csv") {
            log_tx.send(format!("Searching file: {}", path.display()))?;
            
            let file = File::open(&path)?;
            let transcoded = DecodeReaderBytesBuilder::new()
                .encoding(Some(WINDOWS_1252))
                .utf8_passthru(true)
                .build(file);
            let reader = BufReader::new(transcoded);

            for (row_index, line) in reader.lines().enumerate() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        log_tx.send(format!("Error reading line: {}. Skipping...", e))?;
                        continue;
                    }
                };
                
                let fields: Vec<String> = line.split(',')
                    .map(|field| field.trim().to_string())
                    .collect();

                if fields.len() > 2 && (fields[0].to_lowercase() == email.to_lowercase() || 
                                        fields[2].to_lowercase() == email.to_lowercase()) {
                    let result = format!("Found email in file: {}, Row {}: {}", path.display(), row_index + 1, line);
                    log_tx.send(result.clone())?;
                    return Ok(Some(result));
                }
            }
            
            log_tx.send(format!("Finished searching file: {}", path.display()))?;
        }
    }
    
    Ok(None)
}

fn search_email_main(email: &str, folder_path: &PathBuf, log_tx: Sender<String>) -> Result<String, Box<dyn std::error::Error>> {
    log_tx.send("Starting search...".to_string())?;
    
    match search_email_in_folder(email, folder_path, log_tx.clone())? {
        Some(result) => Ok(result),
        None => {
            let message = "Email not found in any CSV file in the selected folder or its subfolders".to_string();
            log_tx.send(message.clone())?;
            Ok(message)
        }
    }
}

