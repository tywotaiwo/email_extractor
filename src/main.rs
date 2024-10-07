use csv::ReaderBuilder;
use csv::Writer;
use eframe::egui;
use rfd::FileDialog;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use egui::{Color32, RichText, Stroke, Rounding};

#[derive(PartialEq)]
enum Theme {
    Light,
    Dark,
}

struct CsvProcessorApp {
    selected_files: Vec<PathBuf>,
    processing_status: String,
    rx: Receiver<String>,
    tx: Sender<String>,
    states: String,
    email_domains: String,
    theme: Theme,
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
                if ui.button(RichText::new("📁 Select CSV Files").size(18.0)).clicked() {
                    if let Some(files) = FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_directory("/")
                        .pick_files()
                    {
                        self.selected_files = files;
                    }
                }
                ui.label(RichText::new(format!("Selected files: {}", self.selected_files.len())).size(16.0));
            });

            ui.add_space(10.0);

            egui::Frame::none()
                .fill(if is_dark { Color32::DARK_GRAY } else { Color32::LIGHT_GRAY })
                .rounding(Rounding::same(8.0))
                .stroke(Stroke::new(1.0, if is_dark { Color32::GRAY } else { Color32::DARK_GRAY }))
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

            if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(RichText::new("🚀 Process Files").size(20.0))).clicked() {
                let files = self.selected_files.clone();
                let tx = self.tx.clone();
                let states: Vec<String> = self.states.split(',').map(|s| s.trim().to_string()).collect();
                let email_domains: Vec<String> = self.email_domains.split(',').map(|s| s.trim().to_string()).collect();
                thread::spawn(move || {
                    for file in files {
                        if let Err(e) = process_csv_file(&file, &states, &email_domains) {
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

            // Check for new messages from the processing thread
            while let Ok(message) = self.rx.try_recv() {
                self.processing_status = message;
            }

            if !self.processing_status.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Status:").strong());
                    ui.label(&self.processing_status);
                });
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    ui.selectable_value(&mut self.theme, Theme::Light, "☀ Light");
                    ui.selectable_value(&mut self.theme, Theme::Dark, "🌙 Dark");
                });
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let (tx, rx) = channel();
    let app = CsvProcessorApp {
        selected_files: Vec::new(),
        processing_status: String::new(),
        rx,
        tx,
        states: "NY,OH,PA,WA,AK".to_string(),
        email_domains: "@gmail.com".to_string(),
        theme: Theme::Light,
    };
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(500.0, 600.0)),
        min_window_size: Some(egui::vec2(400.0, 500.0)),
        ..Default::default()
    };
    eframe::run_native(
        "CSV Processor",
        native_options,
        Box::new(|_cc| Box::new(app)),
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
