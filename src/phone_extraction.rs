use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use egui::RichText;
use rfd::FileDialog;

pub struct PhoneExtractionTab;

impl PhoneExtractionTab {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, selected_files: &mut Vec<PathBuf>, processing_status: &mut String, tx: &Sender<String>) {
        ui.horizontal(|ui| {
            if ui.button(RichText::new("📁 Select CSV Files").size(18.0)).clicked() {
                if let Some(files) = FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_directory("/")
                    .pick_files()
                {
                    *selected_files = files;
                }
            }
            ui.label(RichText::new(format!("Selected files: {}", selected_files.len())).size(16.0));
        });

        ui.add_space(20.0);

        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(RichText::new("📞 Extract Phone Numbers").size(20.0))).clicked() {
            let files = selected_files.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut all_phone_numbers = Vec::new();
                for file in files {
                    match crate::extract_phone_numbers(&file) {
                        Ok(numbers) => {
                            all_phone_numbers.extend(numbers);
                            tx.send(format!("Extracted phone numbers from: {}", file.display())).unwrap();
                        }
                        Err(e) => {
                            tx.send(format!("Error processing {}: {}", file.display(), e)).unwrap();
                        }
                    }
                }
                if let Err(e) = crate::save_phone_numbers_to_file(&all_phone_numbers) {
                    tx.send(format!("Error saving phone numbers: {}", e)).unwrap();
                } else {
                    tx.send("Phone numbers extracted and saved to 'phone_numbers.txt'".to_string()).unwrap();
                }
            });
        }

        ui.add_space(10.0);

        // Display processing status
        if !processing_status.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Status:").strong());
                ui.label(&*processing_status);

            });
        }
    }
}