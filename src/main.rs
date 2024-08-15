#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

use eframe::egui;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::{self, File};
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use rfd::FileDialog;

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 350.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Arca Music",
        options,
        Box::new(|_cc| Ok(Box::<MediaPlayerApp>::default())),
    )
}

struct MediaPlayerApp {
    song_title: String,
    sink: Option<Arc<Mutex<Sink>>>,
    _stream: Option<OutputStream>, // To keep the stream alive
    songs: Vec<PathBuf>,
    selected_song: Option<usize>,
    current_directory: String, // Campo para armazenar o diretório atual
}

impl Default for MediaPlayerApp {
    fn default() -> Self {
        let initial_directory = "D:\\war\\arca_music\\src\\".to_owned(); // Diretório inicial
        let songs = Self::read_songs_from_directory(&initial_directory);
        Self {
            song_title: "No song loaded".to_owned(),
            sink: None,
            _stream: None,
            songs,
            selected_song: None,
            current_directory: initial_directory,
        }
    }
}

impl MediaPlayerApp {
    fn read_songs_from_directory(directory: &str) -> Vec<PathBuf> {
        let mut songs = vec![];
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|ext| ext == "mp3").unwrap_or(false) {
                    songs.push(path);
                }
            }
        }
        songs
    }

    fn load_song(&mut self) {
        if let Some(index) = self.selected_song {
            if let Some(path) = self.songs.get(index) {
                if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                    if let Ok(file) = File::open(path) {
                        let sink = Sink::try_new(&stream_handle).expect("Failed to create Sink");
                        let source = Decoder::new(BufReader::new(file)).expect("Failed to decode audio");
                        sink.append(source);
                        sink.pause(); // Load the song but keep it paused

                        self.song_title = path.to_string_lossy().to_string();
                        self.sink = Some(Arc::new(Mutex::new(sink)));
                        self._stream = Some(stream);
                    } else {
                        self.song_title = "Failed to load song".to_owned();
                    }
                }
            }
        }
    }

    fn update_directory(&mut self) {
        self.songs = Self::read_songs_from_directory(&self.current_directory);
        self.selected_song = None;
    }

    fn open_directory_dialog(&mut self) {
        if let Some(directory) = FileDialog::new().pick_folder() {
            if let Some(dir_str) = directory.to_str() {
                self.current_directory = dir_str.to_owned();
                self.update_directory();
            }
        }
    }

    fn play(&self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().play();
        }
    }

    fn pause(&self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().pause();
        }
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.lock().unwrap().stop();
            self.song_title = "No song loaded".to_owned();
            self._stream = None; // Drop the stream to stop the audio
        }
    }
}

impl eframe::App for MediaPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Arca Music");
            ui.add_space(10.0); // Espaço após o título

            // Botão para abrir o explorador de diretórios
            ui.horizontal(|ui| {
                ui.label("Current Directory:");
                if ui.button("Select Directory").clicked() {
                    self.open_directory_dialog();
                }
                ui.label(&self.current_directory);
            });
            ui.add_space(10.0); // Espaço após o campo de diretório

            // ComboBox para seleção de música
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Select a song")
                    .selected_text(
                        self.selected_song
                            .map(|index| self.songs[index].file_name().unwrap().to_string_lossy())
                            .unwrap_or_else(|| "None".into()),
                    )
                    .show_ui(ui, |ui| {
                        for (index, song) in self.songs.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_song, Some(index), song.file_name().unwrap().to_string_lossy());
                        }
                    });
            });
            ui.add_space(10.0); // Espaço após o ComboBox

            // Botões de controle
            ui.horizontal(|ui| {
                if ui.button("Load").clicked() {
                    self.load_song();
                }

                if ui.button("Play").clicked() {
                    self.play();
                }

                if ui.button("Pause").clicked() {
                    self.pause();
                }

                if ui.button("Stop").clicked() { // Corrigi aqui
                    self.stop();
                }
            });
            ui.add_space(10.0); // Espaço após os botões

            ui.label(format!("Now playing: {}", self.song_title));
        });
    }
}
