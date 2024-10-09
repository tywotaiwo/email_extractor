use csv::ReaderBuilder;
use csv::Writer;
use eframe::egui;
use rfd::FileDialog;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use egui::{Color32, RichText, Stroke, Rounding};
use regex::Regex;
use std::io::Write;

mod csv_processing;
mod phone_extraction;
mod email_search;
mod email_comparison;

use csv_processing::CsvProcessingTab;
use phone_extraction::PhoneExtractionTab;
use email_search::EmailSearchTab;
use email_comparison::EmailComparisonTab;

#[derive(PartialEq)]
enum Theme {
    Light,
    Dark,
}

#[derive(PartialEq)]
enum Tab {
    CsvProcessing,
    PhoneExtraction,
    EmailSearch,
    EmailComparison, // Add this line
}

struct CsvProcessorApp {
    selected_files: Vec<PathBuf>,
    processing_status: String,
    rx: Receiver<String>,
    tx: Sender<String>,
    theme: Theme,
    current_tab: Tab,
    csv_processing_tab: CsvProcessingTab,
    phone_extraction_tab: PhoneExtractionTab,
    email_search_tab: EmailSearchTab,
    email_comparison_tab: EmailComparisonTab, // Add this line
}

impl eframe::App for CsvProcessorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_dark = matches!(self.theme, Theme::Dark);
        ctx.set_visuals(if is_dark { egui::Visuals::dark() } else { egui::Visuals::light() });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading(RichText::new("CSV Processor").size(32.0).color(if is_dark { Color32::LIGHT_BLUE } else { Color32::DARK_BLUE }));
                ui.add_space(20.0);
            });

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::CsvProcessing, "CSV Processing");
                ui.selectable_value(&mut self.current_tab, Tab::PhoneExtraction, "Phone Extraction");
                ui.selectable_value(&mut self.current_tab, Tab::EmailSearch, "Email Search");
                ui.selectable_value(&mut self.current_tab, Tab::EmailComparison, "Email Comparison"); // Add this line
            });

            ui.add_space(10.0);

            match self.current_tab {
                Tab::CsvProcessing => self.csv_processing_tab.ui(ui, &mut self.selected_files, &mut self.processing_status, &self.tx),
                Tab::PhoneExtraction => self.phone_extraction_tab.ui(ui, &mut self.selected_files, &mut self.processing_status, &self.tx),
                Tab::EmailSearch => self.email_search_tab.ui(ui, &mut self.processing_status, &self.tx),
                Tab::EmailComparison => self.email_comparison_tab.ui(ui, &mut self.processing_status, &self.tx), // Add this line
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    ui.selectable_value(&mut self.theme, Theme::Light, "☀ Light");
                    ui.selectable_value(&mut self.theme, Theme::Dark, "🌙 Dark");
                });
            });
        });

        // Check for new messages from the processing thread
        while let Ok(message) = self.rx.try_recv() {
            self.processing_status = message;
        }
    }
}

impl CsvProcessorApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = channel();
        Self {
            selected_files: Vec::new(),
            processing_status: String::new(),
            rx,
            tx,
            theme: Theme::Light,
            current_tab: Tab::EmailSearch, // Changed this line
            csv_processing_tab: CsvProcessingTab::new(),
            phone_extraction_tab: PhoneExtractionTab::new(),
            email_search_tab: EmailSearchTab::new(),
            email_comparison_tab: EmailComparisonTab::new(), // Add this line
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(500.0, 600.0)),
        min_window_size: Some(egui::vec2(400.0, 500.0)),
        ..Default::default()
    };
    eframe::run_native(
        "CSV Processor",
        native_options,
        Box::new(|cc| Box::new(CsvProcessorApp::new(cc))),
    )
}

fn process_csv_file(file_path: &PathBuf, states: &[String], email_domains: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(file);

    let mut writers: Vec<Writer<File>> = states
        .iter()
        .map(|state| {
            let output_file = File::create(format!("output_{}.csv", state)).unwrap();
            Writer::from_writer(output_file)
        })
        .collect();

    // Process each record
    for result in rdr.records() {
        let record = result?;
        
        // Check if any column matches any state
        let state_match = states.iter().enumerate().find(|(_, state)| {
            record.iter().any(|field| field.trim() == *state)
        });

        // If a state matches, check for email domain (if specified)
        if let Some((state_index, _)) = state_match {
            let email_match = email_domains.is_empty() || email_domains.iter().any(|domain| {
                record.iter().any(|field| field.to_lowercase().contains(domain))
            });

            if email_match {
                writers[state_index].write_record(&record)?;
            }
        }

    }

    // Flush all the writers to make sure data is written to files
    for mut writer in writers {
        writer.flush()?;
    }

    Ok(())
}

fn extract_phone_numbers(file_path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(file);
    let phone_regex = Regex::new(r"\(\d{3}\)\s*\d{3}-\d{4}").unwrap();
    let mut phone_numbers = Vec::new();

    for result in rdr.records() {
        let record = result?;
        for field in record.iter() {
            if let Some(phone) = phone_regex.find(field) {
                let formatted_number = format!("+1{}", phone.as_str().replace(&['(', ')', ' ', '-'][..], ""));
                phone_numbers.push(formatted_number);
            }
        }
    }

    Ok(phone_numbers)
}

fn save_phone_numbers_to_file(phone_numbers: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create("phone_numbers.txt")?;
    for number in phone_numbers {
        writeln!(file, "{}", number)?;
    }
    Ok(())
}