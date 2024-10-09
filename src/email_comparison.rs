use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use rfd::FileDialog;

pub struct EmailComparisonTab {
    file1_path: Option<PathBuf>,
    file2_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
}

impl EmailComparisonTab {
    pub fn new() -> Self {
        Self {
            file1_path: None,
            file2_path: None,
            output_path: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, processing_status: &mut String, tx: &Sender<String>) {
        ui.heading("Email Comparison");

        ui.horizontal(|ui| {
            if ui.button("Select First Email List").clicked() {
                if let Some(path) = FileDialog::new().add_filter("Text file", &["txt"]).pick_file() {
                    self.file1_path = Some(path);
                }
            }
            if let Some(path) = &self.file1_path {
                ui.label(format!("First file: {}", path.display()));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Select Second Email List").clicked() {
                if let Some(path) = FileDialog::new().add_filter("Text file", &["txt"]).pick_file() {
                    self.file2_path = Some(path);
                }
            }
            if let Some(path) = &self.file2_path {
                ui.label(format!("Second file: {}", path.display()));
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Select Output File").clicked() {
                if let Some(path) = FileDialog::new().add_filter("Text file", &["txt"]).save_file() {
                    self.output_path = Some(path);
                }
            }
            if let Some(path) = &self.output_path {
                ui.label(format!("Output file: {}", path.display()));
            }
        });

        if ui.button("Compare and Output Unique Emails").clicked() {
            if let (Some(file1), Some(file2), Some(output)) = (&self.file1_path, &self.file2_path, &self.output_path) {
                match self.compare_email_lists(file1, file2, output) {
                    Ok(unique_count) => {
                        *processing_status = format!("Comparison complete. {} unique emails found.", unique_count);
                    }
                    Err(e) => {
                        *processing_status = format!("Error during comparison: {}", e);
                    }
                }
            } else {
                *processing_status = "Please select both input files and an output file.".to_string();
            }
        }
    }

    fn compare_email_lists(&self, file1: &PathBuf, file2: &PathBuf, output: &PathBuf) -> Result<usize, Box<dyn std::error::Error>> {
        let emails1 = self.read_emails(file1)?;
        let emails2 = self.read_emails(file2)?;

        let unique_emails: HashSet<_> = emails1.symmetric_difference(&emails2).collect();

        let mut output_file = File::create(output)?;
        for email in &unique_emails {
            writeln!(output_file, "{}", email)?;
        }
        Ok(unique_emails.len())
    }

    fn read_emails(&self, file_path: &PathBuf) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let emails: HashSet<String> = reader.lines()
            .filter_map(Result::ok)
            .map(|line| line.trim().to_lowercase())
            .filter(|line| !line.is_empty())
            .collect();
        Ok(emails)
    }
}