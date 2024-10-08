use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use egui::{RichText, Stroke, Rounding};
use rfd::FileDialog;

pub struct CsvProcessingTab {
    states: String,
    email_domains: String,
}

impl CsvProcessingTab {
    pub fn new() -> Self {
        Self {
            states: "NY,OH,PA,WA,AK".to_string(),
            email_domains: "@gmail.com".to_string(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, selected_files: &mut Vec<PathBuf>, processing_status: &mut String, tx: &Sender<String>) {
        // File selection UI
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

        ui.add_space(10.0);

        // States and email domains input
        egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .rounding(Rounding::same(8.0))
            .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color))
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("States:").size(16.0));
                    ui.add(egui::TextEdit::singleline(&mut self.states).hint_text("NY, OH, PA, WA, AK"));
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Email domains:").size(16.0));
                    ui.add(egui::TextEdit::singleline(&mut self.email_domains).hint_text("@gmail.com, @yahoo.com"));
                });
                ui.add_space(10.0);
            });

        ui.add_space(20.0);

        // Process files button
        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(RichText::new("🚀 Process Files").size(20.0))).clicked() {
            let files = selected_files.clone();
            let tx = tx.clone();
            let states: Vec<String> = self.states.split(',').map(|s| s.trim().to_string()).collect();
            let email_domains: Vec<String> = self.email_domains.split(',').map(|s| s.trim().to_string()).collect();
            thread::spawn(move || {
                for file in files {
                    if let Err(e) = crate::process_csv_file(&file, &states, &email_domains) {
                        tx.send(format!("Error processing {}: {}", file.display(), e))
                            .unwrap();
                    } else {
                        tx.send(format!("Processed: {}", file.display())).unwrap();
                    }
                }
                tx.send("All files processed".to_string()).unwrap();
            });
        }

        ui.add_space(10.0);

        // Display processing status
        if !processing_status.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Status:").strong());
                ui.label(&*processing_status.clone());
            });
        }
    }
}