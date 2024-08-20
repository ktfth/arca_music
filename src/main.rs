#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

use dirs::home_dir;
use eframe::egui::{self, Slider, Layout, Align, Direction};
use id3::Tag;
use id3::TagLike;
use rfd::FileDialog;
use rodio::Source;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    song_finished: Arc<AtomicBool>, // Indica se a música terminou
    layout: LayoutSettings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
struct LayoutSettings {
    main_dir: Direction,
    main_wrap: bool,
    cross_align: Align,
    cross_justify: bool,
}

trait LayoutSettingsExt {
    fn layout(&self) -> egui::Layout;
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self::top_down_justified()
    }
}

impl LayoutSettings {
    fn top_down_justified() -> Self {
        Self {
            main_dir: Direction::TopDown,
            main_wrap: false,
            cross_align: Align::Center,
            cross_justify: true,
        }
    }
}

impl LayoutSettingsExt for LayoutSettings {
    fn layout(&self) -> egui::Layout {
        Layout::from_main_dir_and_cross_align(self.main_dir, self.cross_align)
            .with_main_wrap(self.main_wrap)
            .with_cross_justify(self.cross_justify)
    }
}

impl Default for MediaPlayerApp {
    fn default() -> Self {
        let initial_directory = home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string(); // Diretório inicial
        let songs = Self::read_songs_from_directory(&initial_directory);
        let (_stream, stream_handle) = OutputStream::try_default().unwrap(); // Mantém o stream durante toda a vida útil
        Self {
            song_title: "Song Title".to_owned(),
            artist_name: "Unknown Artist".to_owned(),
            sink: Some(Arc::new(Mutex::new(Sink::try_new(&stream_handle).unwrap()))),
            _stream: Some(_stream), // Mantém o stream vivo
            current_time: 0.0,
            total_time: 200.0, // Exemplo de tempo total em segundos (e.g., 3:20)
            start_time: None,
            volume: 0.5,
            songs,
            selected_song: None,
            current_directory: initial_directory,
            is_playing: false,
            song_finished: Arc::new(AtomicBool::new(false)),
            layout: LayoutSettings::default(),
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

    fn previous_song(&mut self) {
        if let Some(current_index) = self.selected_song {
            if current_index > 0 {
                self.selected_song = Some(current_index - 1);
                self.load_and_play_song();
            }
        }
    }

    fn next_song(&mut self) {
        if let Some(current_index) = self.selected_song {
            if current_index + 1 < self.songs.len() {
                self.selected_song = Some(current_index + 1);
                self.load_and_play_song();
            } else {
                self.stop(); // Se for a última música, pare a reprodução
            }
        }
    }

    fn load_and_play_song(&mut self) {
        self.stop_current_song(); // Pare a música atual
        self.load_song(); // Carrega a nova música
        self.play(); // Reproduz a nova música
    }

    fn play(&mut self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().play();
            sink.lock().unwrap().set_volume(self.volume); // Ajusta o volume na reprodução
            self.start_time = Some(std::time::Instant::now());
            self.is_playing = true;
        }
    }

    fn stop(&mut self) {
        self.stop_current_song();
    }

    fn stop_current_song(&mut self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().stop();
        }
        self.is_playing = false;
        self.current_time = 0.0;
    }

    fn load_song(&mut self) {
        if let Some(index) = self.selected_song {
            if let Some(path) = self.songs.get(index) {
                if let Some(sink) = &self.sink {
                    if let Ok(file) = File::open(path) {
                        // Carregando os metadados ID3
                        if let Ok(tag) = Tag::read_from_path(path) {
                            self.song_title = tag.title().unwrap_or("Unknown Title").to_string();
                            self.artist_name = tag.artist().unwrap_or("Unknown Artist").to_string();
                        } else {
                            self.song_title =
                                path.file_name().unwrap().to_string_lossy().to_string();
                            self.artist_name = "Unknown Artist".to_owned();
                        }

                        match mp3_duration::from_file(&File::open(path).unwrap()) {
                            Ok(duration) => {
                                self.total_time = duration.as_secs_f32();
                            }
                            Err(e) => {
                                eprintln!("Erro ao calcular a duração do MP3: {:?}", e);
                                self.total_time = 0.0;
                            }
                        }

                        let source =
                            Decoder::new(BufReader::new(file)).expect("Failed to decode audio");

                        self.total_time = source
                            .total_duration()
                            .map(|d| d.as_secs_f32())
                            .unwrap_or(0.0); // Obtém a duração real da música

                        sink.lock().unwrap().append(source);
                        self.current_time = 0.0;
                        self.song_finished.store(false, Ordering::SeqCst); // Reseta o estado de término da música
                    } else {
                        self.song_title = "Failed to load song".to_owned();
                        self.artist_name = "Unknown Artist".to_owned();
                    }
                }
            }
        }
    }

    fn check_song_finished(&mut self) {
        if self.song_finished.load(Ordering::SeqCst) {
            self.song_finished.store(false, Ordering::SeqCst); // Reseta o indicador
            self.next_song(); // Avança para a próxima música
        }
    }

    fn update_directory(&mut self) {
        if let Some(directory) = FileDialog::new().pick_folder() {
            if let Some(dir_str) = directory.to_str() {
                self.current_directory = dir_str.to_owned();
                self.songs = Self::read_songs_from_directory(&self.current_directory); // Carrega as músicas do novo diretório
                self.selected_song = None; // Reseta a seleção
            }
        }
    }

    fn update_progress(&mut self, value: f32) {
        self.current_time = value;
        // Aqui você pode implementar a lógica para buscar na stream de áudio
    }

    fn update_time(&mut self) {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f32();

            // Se o áudio é muito curto, normalize a atualização
            let normalized_elapsed = if self.total_time <= 10.0 {
                // Para áudios com 10 segundos ou menos, normaliza o tempo
                elapsed * (self.total_time / 10.0)
            } else {
                elapsed
            };

            self.current_time = (self.current_time + normalized_elapsed).min(self.total_time);
            self.start_time = Some(std::time::Instant::now()); // Reinicia o temporizador
        }
    }

    fn adjust_volume(&mut self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().set_volume(self.volume);
        }
    }

    fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().pause();
            self.is_playing = false;
            self.start_time = None;
        }
    }

    fn seek(&mut self, position: f32) {
        if let Some(index) = self.selected_song {
            if let Some(path) = self.songs.get(index) {
                if let Some(sink) = &self.sink {
                    sink.lock().unwrap().stop(); // Para a reprodução atual

                    if let Ok(file) = File::open(path) {
                        let source =
                            Decoder::new(BufReader::new(file)).expect("Failed to decode audio");

                        // Avança a posição no fluxo de áudio
                        let skipped_source =
                            source.skip_duration(std::time::Duration::from_secs_f32(position));

                        self.current_time = position.min(self.total_time); // Atualiza o tempo atual

                        // Adiciona a nova fonte ao sink e retoma a reprodução
                        sink.lock().unwrap().append(skipped_source);
                        sink.lock().unwrap().play();
                    }
                }
            }
        }
    }
}

impl eframe::App for MediaPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_time();
        self.check_song_finished();

        egui::SidePanel::left("side_panel")
            .default_width(225.0) // Define a largura padrão do painel para 200 pixels
            .min_width(175.0) // Define a largura mínima do painel para 150 pixels
            .show(ctx, |ui| {
                ui.heading("Music List");
                ui.add_space(10.0);

                if !self.songs.is_empty() {
                    let mut selected_index = None;

                    ui.vertical(|ui| {
                        for (index, song) in self.songs.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    self.selected_song == Some(index),
                                    song.file_name().unwrap().to_string_lossy(),
                                )
                                .clicked()
                            {
                                selected_index = Some(index);
                            }
                        }
                    });

                    if let Some(index) = selected_index {
                        self.selected_song = Some(index);
                        self.load_and_play_song(); // Carrega a música selecionada
                    }
                } else {
                    ui.label("No songs available.");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Grid::new("main_grid")
                .striped(true)
                .min_col_width(ui.available_width()) // Para garantir que o grid ocupe a largura total
                .show(ui, |ui| {
                    ui.with_layout(
                        self.layout.layout(),
                        |ui| {
                            ui.add_space(10.0);
                            ui.heading("Arca Music");
                            ui.add_space(20.0);
                        },
                    );
                    ui.end_row();

                    ui.with_layout(self.layout.layout(), |ui| {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label("Current Directory:");
                            if ui.button("Select Directory").clicked() {
                                self.update_directory(); // Atualiza o diretório e lista as músicas
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(&self.current_directory);
                        });
                        ui.add_space(5.0); // Espaço após o campo de diretório
                    });
                    ui.end_row();

                    ui.with_layout(self.layout.layout(), |ui| {
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.heading(&self.artist_name);
                                ui.label(&self.song_title);
                            });
                        });
                        ui.add_space(20.0);
                    });
                    ui.end_row();

                    ui.with_layout(self.layout.layout(), |ui| {
                        ui.add_space(15.0); // Espaço antes do controle de progresso
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{:02}:{:02}",
                                self.current_time as i32 / 60,
                                self.current_time as i32 % 60
                            ));
                            if ui
                                .add(
                                    Slider::new(&mut self.current_time, 0.0..=self.total_time)
                                        .show_value(false)
                                        .trailing_fill(true),
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
                        ui.add_space(10.0); // Espaço antes dos controles
                    });
                    ui.end_row();

                    ui.with_layout(self.layout.layout(), |ui| {
                        ui.add_space(25.0); // Espaço antes dos controles
                        ui.horizontal(|ui| {
                            // let button_width = 78.0; // Divide a largura disponível igualmente entre os 4 botões
                            let button_width = ui.available_width() / 4.0 - 6.0; // Divide a largura disponível igualmente entre os 4 botões
                            
                            if ui
                                .add_sized([button_width, 30.0], egui::Button::new("Back"))
                                .clicked()
                            {
                                self.previous_song(); // Lógica para voltar à música anterior
                            }

                            if ui
                                .add_sized([button_width, 30.0], egui::Button::new("Play"))
                                .clicked()
                            {
                                if self.is_playing {
                                    self.pause(); // Pausa a música se estiver tocando
                                } else {
                                    self.play(); // Reproduz a música se estiver pausada
                                }
                            }

                            if ui
                                .add_sized([button_width, 30.0], egui::Button::new("Next"))
                                .clicked()
                            {
                                self.update_progress(0.0);
                                self.stop(); // Parar a música atual
                                self.next_song(); // Avançar para a próxima música
                            }

                            if ui
                                .add_sized([button_width, 30.0], egui::Button::new("Stop"))
                                .clicked()
                            {
                                self.stop(); // Lógica para parar a música
                            }
                        });
                        ui.add_space(20.0); // Espaço antes do controle de volume
                    });
                    ui.end_row();

                    ui.with_layout(self.layout.layout(), |ui| {
                        ui.horizontal(|ui| {
                            ui.label("🔈");
                            if ui
                                .add(Slider::new(&mut self.volume, 0.0..=1.0).show_value(true))
                                .changed()
                            {
                                self.adjust_volume(); // Ajusta o volume ao alterar o slider
                            }
                            ui.label("🔊");
                        });
                    });
                    ui.end_row();
                });
        });

        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([500.0, 450.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Arca Music",
        options,
        Box::new(|_cc| Ok(Box::<MediaPlayerApp>::default())),
    )
}
