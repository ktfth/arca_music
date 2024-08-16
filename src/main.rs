#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

use eframe::egui::{self, Slider};
use egui::{Align, Direction, Layout};
use id3::Tag;
use id3::TagLike;
use rfd::FileDialog;
use rodio::Source;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct MediaPlayerApp {
    song_title: String,
    artist_name: String,
    sink: Option<Arc<Mutex<Sink>>>,
    _stream: Option<OutputStream>, // To keep the stream alive
    current_time: f32,
    total_time: f32,
    start_time: Option<std::time::Instant>,
    volume: f32,
    songs: Vec<PathBuf>,
    selected_song: Option<usize>,
    current_directory: String, // Campo para armazenar o diretório atual
    is_playing: bool,          // Estado de reprodução
}

impl Default for MediaPlayerApp {
    fn default() -> Self {
        let initial_directory = "D:\\war\\arca_music\\src\\".to_owned(); // Diretório inicial
        let songs = Self::read_songs_from_directory(&initial_directory);
        Self {
            song_title: "Song Title".to_owned(),
            artist_name: "Unknown Artist".to_owned(),
            sink: None,
            _stream: None,
            current_time: 0.0,
            total_time: 200.0, // Exemplo de tempo total em segundos (e.g., 3:20)
            start_time: None,
            volume: 0.5,
            songs,
            selected_song: None,
            current_directory: initial_directory,
            is_playing: false,
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

    fn load_and_play_song(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.load_song();
            self.play();
        }
    }

    fn load_song(&mut self) {
        if let Some(index) = self.selected_song {
            if let Some(path) = self.songs.get(index) {
                if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                    if let Ok(file) = File::open(path) {
                        // Carregando os metadados ID3
                        if let Ok(tag) = Tag::read_from_path(path) {
                            self.song_title = tag.title().unwrap_or("Unknown Title").to_string();
                            self.artist_name = tag.artist().unwrap_or("Unknown Artist").to_string();
                            // Outros metadados podem ser carregados aqui, como álbum, ano, etc.
                        } else {
                            self.song_title =
                                path.file_name().unwrap().to_string_lossy().to_string();
                            self.artist_name = "Unknown Artist".to_owned();
                        }

                        let source =
                            Decoder::new(BufReader::new(file)).expect("Failed to decode audio");

                        self.total_time = source
                            .total_duration()
                            .map(|d| d.as_secs_f32())
                            .unwrap_or(0.0); // Obtém a duração real da música

                        let sink = Sink::try_new(&stream_handle).expect("Failed to create Sink");
                        sink.append(source);
                        sink.pause(); // Carrega a música, mas mantém pausada

                        self.sink = Some(Arc::new(Mutex::new(sink)));
                        self._stream = Some(stream);
                        self.current_time = 0.0;
                    } else {
                        self.song_title = "Failed to load song".to_owned();
                        self.artist_name = "Unknown Artist".to_owned();
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

    fn update_progress(&mut self, value: f32) {
        self.current_time = value;
        // Aqui você pode implementar a lógica para buscar na stream de áudio
    }

    fn seek(&mut self, position: f32) {
        if let Some(_sink) = &self.sink {
            let elapsed_time = position.min(self.total_time);
            self.current_time = elapsed_time;

            // Neste ponto, ajustamos a posição desejada, mas mantemos a reprodução contínua.
            // No entanto, rodio não suporta diretamente o seek. Aqui estamos simulando
            // o comportamento aproximado ao ajustar o tempo corrente.
        }
    }

    fn update_time(&mut self) {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f32();
            self.current_time = (self.current_time + elapsed).min(self.total_time);
            self.start_time = Some(std::time::Instant::now()); // Reinicia o temporizador
        }
    }

    fn adjust_volume(&mut self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().set_volume(self.volume);
        }
    }

    fn play(&mut self) {
        self.start_time = Some(std::time::Instant::now());
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().play();
            self.is_playing = true;
        }
    }

    fn pause(&mut self) {
        self.start_time = None;
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().pause();
            self.is_playing = false;
        }
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.lock().unwrap().stop();
            self.current_time = 0.0;
            self.is_playing = false;
            self._stream = None; // Liberar o stream para parar o áudio
        }
    }

    fn layout(&mut self) -> egui::Layout {
        egui::Layout::top_down_justified(egui::Align::Center).with_cross_justify(true)
    }
}

impl eframe::App for MediaPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_time();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(self.layout(), |ui| {
                ui.add_space(10.0);
                ui.heading("Arca Music");
                ui.add_space(20.0);
            });

            ui.with_layout(self.layout(), |ui| {
                // Diretório atual e botão de seleção de diretório
                ui.horizontal(|ui| {
                    ui.label("Current Directory:");
                    if ui.button("Select Directory").clicked() {
                        self.open_directory_dialog();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(&self.current_directory);
                });

                ui.add_space(20.0); // Espaço após o campo de diretório
            });

            ui.with_layout(self.layout(), |ui| {
                // ComboBox para seleção de música
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Select a song")
                        .selected_text(
                            self.selected_song
                                .map(|index| {
                                    self.songs[index].file_name().unwrap().to_string_lossy()
                                })
                                .unwrap_or_else(|| "Select a song".into()),
                        )
                        .show_ui(ui, |ui| {
                            for (index, song) in self.songs.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_song,
                                    Some(index),
                                    song.file_name().unwrap().to_string_lossy(),
                                );
                            }
                        });
                });

                ui.add_space(20.0); // Espaço após o ComboBox
            });

            ui.with_layout(self.layout(), |ui| {
                // Informações da música e barra de progresso
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(&self.artist_name);
                        ui.label(&self.song_title);
                    });
                });

                ui.add_space(10.0);
            });

            ui.with_layout(self.layout(), |ui| {
                // Barra de progresso
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{:02}:{:02}",
                        self.current_time as i32 / 60,
                        self.current_time as i32 % 60
                    ));
                    if ui
                        .add(
                            Slider::new(&mut self.current_time, 0.0..=self.total_time)
                                .show_value(false),
                        )
                        .changed()
                    {
                        self.seek(self.current_time); // Atualiza o progresso
                    }
                    ui.label(format!(
                        "{:02}:{:02}",
                        self.total_time as i32 / 60,
                        self.total_time as i32 % 60
                    ));
                });

                ui.add_space(20.0); // Espaço antes dos controles
            });

            ui.with_layout(self.layout(), |ui| {
                // Controles de reprodução
                ui.horizontal(|ui| {
                    if ui.button("Backward").clicked() {
                        self.update_progress(0.0); // Voltar ao início
                    }
                    if ui
                        .button(if self.start_time.is_some() {
                            "Pause"
                        } else {
                            "Play"
                        })
                        .clicked()
                    {
                        if self.start_time.is_some() {
                            self.pause();
                        } else {
                            self.load_and_play_song();
                        }
                    }

                    if ui.button("Forward").clicked() {
                        self.update_progress(self.total_time); // Avançar para o final
                    }
                    if ui.button("Stop").clicked() {
                        self.stop(); // Parar a reprodução
                    }
                });

                ui.add_space(20.0); // Espaço antes do controle de volume
            });

            ui.with_layout(self.layout(), |ui| {
                // Controle de volume
                ui.horizontal(|ui| {
                    ui.label("🔈");
                    if ui
                        .add(Slider::new(&mut self.volume, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        self.adjust_volume();
                    }
                    ui.label("🔊");
                });
            });
        });

        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 350.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Arca Music",
        options,
        Box::new(|_cc| Ok(Box::<MediaPlayerApp>::default())),
    )
}
