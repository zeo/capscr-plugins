// v0.4: capscr's plugin runtime arrives in v0.4. The `runtime` feature flips
// the `Plugin` trait impl + the path dep to `capscr` (Cargo.toml). With the
// feature off the crate compiles as plain reference code — no plugin host
// involvement. Suppress dead-code warnings for runtime-only helpers so the
// standalone build stays warning-clean.
#![cfg_attr(not(feature = "runtime"), allow(dead_code))]

#[cfg(feature = "runtime")]
use capscr::plugin::{CaptureType, Plugin, PluginEvent, PluginResponse};
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundsConfig {
    pub enabled: bool,
    pub volume: f32,
    pub sounds: SoundEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundEvents {
    pub pre_capture: Option<SoundEntry>,
    pub post_capture: Option<SoundEntry>,
    pub post_save: Option<SoundEntry>,
    pub post_upload: Option<SoundEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundEntry {
    pub path: PathBuf,
    pub volume: Option<f32>,
    pub only_modes: Option<Vec<String>>,
}

impl Default for SoundsConfig {
    fn default() -> Self {
        let sounds_dir = directories::ProjectDirs::from("", "", "capscr")
            .map(|dirs| dirs.config_dir().join("plugins").join("sounds"))
            .unwrap_or_else(|| PathBuf::from("sounds"));

        Self {
            enabled: true,
            volume: 1.0,
            sounds: SoundEvents {
                pre_capture: None,
                post_capture: Some(SoundEntry {
                    path: sounds_dir.join("capture.wav"),
                    volume: None,
                    only_modes: None,
                }),
                post_save: Some(SoundEntry {
                    path: sounds_dir.join("save.wav"),
                    volume: Some(0.8),
                    only_modes: None,
                }),
                post_upload: Some(SoundEntry {
                    path: sounds_dir.join("upload.wav"),
                    volume: None,
                    only_modes: None,
                }),
            },
        }
    }
}

enum SoundCommand {
    Play { path: PathBuf, volume: f32 },
    Shutdown,
}

pub struct SoundsPlugin {
    config: SoundsConfig,
    config_path: PathBuf,
    sound_sender: Option<mpsc::Sender<SoundCommand>>,
    sound_thread: Option<thread::JoinHandle<()>>,
}

impl SoundsPlugin {
    pub fn new() -> Self {
        let config_path = Self::default_config_path();
        let config = Self::load_config(&config_path).unwrap_or_default();
        Self {
            config,
            config_path,
            sound_sender: None,
            sound_thread: None,
        }
    }

    pub fn with_config(config: SoundsConfig) -> Self {
        Self {
            config,
            config_path: Self::default_config_path(),
            sound_sender: None,
            sound_thread: None,
        }
    }

    fn default_config_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "capscr")
            .map(|dirs| dirs.config_dir().join("plugins").join("sounds.toml"))
            .unwrap_or_else(|| PathBuf::from("sounds.toml"))
    }

    fn load_config(path: &PathBuf) -> Option<SoundsConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    fn save_config(&self) {
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = toml::to_string_pretty(&self.config) {
            let _ = std::fs::write(&self.config_path, content);
        }
    }

    fn start_sound_thread(&mut self) {
        let (tx, rx) = mpsc::channel::<SoundCommand>();
        self.sound_sender = Some(tx);

        let handle = thread::spawn(move || {
            let stream_result = OutputStream::try_default();
            let (_stream, stream_handle) = match stream_result {
                Ok((stream, handle)) => (stream, handle),
                Err(_) => return,
            };

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    SoundCommand::Play { path, volume } => {
                        if let Ok(file) = File::open(&path) {
                            let reader = BufReader::new(file);
                            if let Ok(source) = Decoder::new(reader) {
                                if let Ok(sink) = Sink::try_new(&stream_handle) {
                                    sink.set_volume(volume);
                                    sink.append(source);
                                    sink.detach();
                                }
                            }
                        }
                    }
                    SoundCommand::Shutdown => break,
                }
            }
        });

        self.sound_thread = Some(handle);
    }

    fn stop_sound_thread(&mut self) {
        if let Some(sender) = self.sound_sender.take() {
            let _ = sender.send(SoundCommand::Shutdown);
        }
        if let Some(handle) = self.sound_thread.take() {
            let _ = handle.join();
        }
    }

    #[cfg(feature = "runtime")]
    fn play_sound(&self, entry: &SoundEntry, mode: Option<&CaptureType>) {
        if !self.config.enabled {
            return;
        }

        if let Some(ref only_modes) = entry.only_modes {
            if let Some(mode) = mode {
                let mode_str = match mode {
                    CaptureType::FullScreen => "fullscreen",
                    CaptureType::Window => "window",
                    CaptureType::Region => "region",
                    CaptureType::Gif => "gif",
                };
                if !only_modes.iter().any(|m| m.to_lowercase() == mode_str) {
                    return;
                }
            }
        }

        if !entry.path.exists() {
            return;
        }

        let volume = entry.volume.unwrap_or(1.0) * self.config.volume;
        let volume = volume.clamp(0.0, 2.0);

        if let Some(ref sender) = self.sound_sender {
            let _ = sender.send(SoundCommand::Play {
                path: entry.path.clone(),
                volume,
            });
        }
    }

    fn create_default_sounds_dir(&self) {
        let sounds_dir = directories::ProjectDirs::from("", "", "capscr")
            .map(|dirs| dirs.config_dir().join("plugins").join("sounds"))
            .unwrap_or_else(|| PathBuf::from("sounds"));

        let _ = std::fs::create_dir_all(&sounds_dir);
    }
}

impl Default for SoundsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "runtime")]
impl Plugin for SoundsPlugin {
    fn name(&self) -> &str {
        "Sounds"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Play customizable sounds on capture events"
    }

    fn on_event(&mut self, event: &PluginEvent) -> PluginResponse {
        match event {
            PluginEvent::PreCapture { mode } => {
                if let Some(ref entry) = self.config.sounds.pre_capture {
                    self.play_sound(entry, Some(mode));
                }
            }
            PluginEvent::PostCapture { mode, .. } => {
                if let Some(ref entry) = self.config.sounds.post_capture {
                    self.play_sound(entry, Some(mode));
                }
            }
            PluginEvent::PostSave { .. } => {
                if let Some(ref entry) = self.config.sounds.post_save {
                    self.play_sound(entry, None);
                }
            }
            PluginEvent::PostUpload { .. } => {
                if let Some(ref entry) = self.config.sounds.post_upload {
                    self.play_sound(entry, None);
                }
            }
            _ => {}
        }
        PluginResponse::Continue
    }

    fn on_load(&mut self) {
        self.create_default_sounds_dir();
        if !self.config_path.exists() {
            self.save_config();
        }
        self.start_sound_thread();
    }

    fn on_unload(&mut self) {
        self.stop_sound_thread();
        self.save_config();
    }
}

impl Drop for SoundsPlugin {
    fn drop(&mut self) {
        self.stop_sound_thread();
    }
}
