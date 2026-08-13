pub mod audio;
pub mod capture;
pub mod control;
pub mod mapping;
pub mod stats;

use crate::backend::audio::{Audio, AudioControl};
use crate::backend::capture::{Capture, CaptureError, KeyEvent, KeyEventKind, KeyEventSource};
use crate::backend::mapping::Mapping;
use crate::backend::stats::StatsStore;
use crate::config::{AppConfig, clamp_volume};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

const RECONNECT_BASE_MS: u64 = 500;
const RECONNECT_MAX_MS: u64 = 10_000;
const MUTE_POLL_MS: u64 = 100;
const UNDERRUN_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("could not start the audio engine: {0}")]
    Audio(#[from] audio::AudioError),
    #[error("could not resolve the control socket path: {0}")]
    SocketPath(String),
    #[error("could not serve the control socket {}: {source}", path.display())]
    ServeSocket {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendStatus {
    pub soundpack: Option<String>,
    pub volume: f32,
    pub device: Option<String>,
    pub device_connected: bool,
    pub stream_failed: bool,
    pub modifier_sounds: bool,
    pub key_up_sounds: bool,
    pub key_up_fallback: bool,
    pub pitch_variation: f32,
    pub velocity_variation: f32,
    pub return_ding: bool,
    pub output_device: Option<String>,
    pub tone_pan: f32,
    pub tone_distance: f32,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct BackendState {
    pub mapping: Mutex<Option<Mapping>>,
    pub config: Mutex<AppConfig>,
}

pub struct BackendInner {
    pub audio: Arc<dyn AudioControl>,
    pub state: BackendState,
    pub config_path: PathBuf,
    pub desired_device: Mutex<Option<String>>,
    pub events: Sender<EngineEvent>,
    pub connected: AtomicBool,
    pub modifier_sounds: AtomicBool,
    pub key_up_sounds: AtomicBool,
    pub key_up_fallback: AtomicBool,
    pub pitch_variation: AtomicU32,
    pub velocity_variation: AtomicU32,
    pub return_ding: AtomicBool,
    pub enabled: AtomicBool,
    pub ding: Mutex<Option<Arc<audio::DecodedSound>>>,
    pub stats: Mutex<StatsStore>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EngineEvent {
    DeviceChanged,
    Shutdown,
}

pub fn run(config_path: &Path, config: AppConfig) -> Result<(), BackendError> {
    let config = AppConfig {
        volume: clamp_volume(config.volume),
        ..config
    };
    let audio = Arc::new(Audio::new(config.volume)?);
    audio.set_tone(config.tone_pan, config.tone_distance);
    audio.set_variation(config.pitch_variation, config.velocity_variation);

    if let Some(device_name) = config.output_device.as_deref()
        && let Err(error) = audio.set_output_device(Some(device_name))
    {
        eprintln!("could not select the saved output device {device_name}: {error}");
    }

    let mapping = config
        .selected_soundpack
        .as_deref()
        .map(|path| match Mapping::load(path) {
            Ok(mapping) => Some(mapping),
            Err(error) => {
                eprintln!("could not load the saved soundpack: {error}");
                None
            }
        })
        .unwrap_or(None);

    let (events, receiver) = mpsc::channel();
    let socket_path = control::socket_path().map_err(BackendError::SocketPath)?;
    let desired_device = config_device_name(&config);
    let modifier_sounds = config.modifier_sounds;
    let key_up_sounds = config.key_up_sounds;
    let key_up_fallback = config.key_up_fallback;
    let pitch_variation = config.pitch_variation;
    let velocity_variation = config.velocity_variation;
    let return_ding = config.return_ding;
    let stats_store = StatsStore::load_or_default(default_stats_path());
    let report_underruns = underrun_reporter(Arc::clone(&audio));
    let backend = Arc::new(BackendInner {
        audio,
        state: BackendState {
            mapping: Mutex::new(mapping),
            config: Mutex::new(config),
        },
        config_path: config_path.to_path_buf(),
        desired_device: Mutex::new(desired_device),
        events,
        connected: AtomicBool::new(false),
        modifier_sounds: AtomicBool::new(modifier_sounds),
        key_up_sounds: AtomicBool::new(key_up_sounds),
        key_up_fallback: AtomicBool::new(key_up_fallback),
        pitch_variation: AtomicU32::new(pitch_variation.to_bits()),
        velocity_variation: AtomicU32::new(velocity_variation.to_bits()),
        return_ding: AtomicBool::new(return_ding),
        enabled: AtomicBool::new(true),
        ding: Mutex::new(None),
        stats: Mutex::new(stats_store),
    });

    let engine_backend = Arc::clone(&backend);
    let engine_thread = std::thread::spawn(move || {
        run_engine(
            &engine_backend,
            receiver,
            |name| Capture::open(name).ok(),
            std::thread::sleep,
            report_underruns,
        )
    });

    let result = control::serve(&socket_path, &backend);

    let _ = backend.events.send(EngineEvent::Shutdown);
    let _ = engine_thread.join();

    result.map_err(|source| BackendError::ServeSocket {
        path: socket_path,
        source,
    })
}

fn config_device_name(config: &AppConfig) -> Option<String> {
    config.device_name.clone()
}

fn underrun_reporter(audio: Arc<Audio>) -> impl FnMut() {
    let mut last_count = 0u32;

    move || {
        let current = audio.underrun_count();
        let delta = current.wrapping_sub(last_count);

        if delta != 0 {
            eprintln!("audio dropout: {delta} underrun(s) detected in the last few seconds");
        }

        last_count = current;
    }
}

fn run_engine<S: KeyEventSource>(
    backend: &Arc<BackendInner>,
    events: Receiver<EngineEvent>,
    mut open: impl FnMut(&str) -> Option<S>,
    mut sleep: impl FnMut(Duration),
    mut report_underruns: impl FnMut(),
) {
    let mut capture: Option<S> = None;
    let mut backoff_ms = RECONNECT_BASE_MS;
    let mut reopen = false;
    let mut last_underrun_poll = Instant::now();

    loop {
        if last_underrun_poll.elapsed() >= UNDERRUN_POLL_INTERVAL {
            report_underruns();
            last_underrun_poll = Instant::now();
        }

        loop {
            match events.try_recv() {
                Ok(EngineEvent::Shutdown) => return,
                Ok(EngineEvent::DeviceChanged) => reopen = true,
                Err(_) => break,
            }
        }

        if reopen {
            reopen = false;
            capture = None;
        }

        if !backend.enabled.load(Ordering::Relaxed) {
            capture = None;
            backend.connected.store(false, Ordering::Relaxed);
            sleep(Duration::from_millis(MUTE_POLL_MS));
            continue;
        }

        if capture.is_none() {
            let device = backend
                .desired_device
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();

            capture = match device.as_deref().and_then(&mut open) {
                Some(opened) => {
                    backoff_ms = RECONNECT_BASE_MS;
                    backend.connected.store(true, Ordering::Relaxed);
                    Some(opened)
                }
                None => {
                    backend.connected.store(false, Ordering::Relaxed);
                    sleep(Duration::from_millis(backoff_ms));
                    backoff_ms = (backoff_ms * 2).min(RECONNECT_MAX_MS);
                    continue;
                }
            };
        }

        let Some(active) = capture.as_mut() else {
            continue;
        };

        match active.next_key_event() {
            Ok(Some(event)) => fire(backend, event),
            Ok(None) => {}
            Err(CaptureError::DeviceGone { .. }) => {
                backend.connected.store(false, Ordering::Relaxed);
                capture = None;
            }
            Err(error) => {
                eprintln!("keyboard capture error: {error}");
                backend.connected.store(false, Ordering::Relaxed);
                capture = None;
            }
        }
    }
}

fn fire(backend: &BackendInner, event: KeyEvent) {
    if !backend.enabled.load(Ordering::Relaxed) {
        return;
    }
    let modifier_sounds = backend.modifier_sounds.load(Ordering::Relaxed);
    let key_up_sounds = backend.key_up_sounds.load(Ordering::Relaxed);
    let key_up_fallback = backend.key_up_fallback.load(Ordering::Relaxed);
    let return_ding = backend.return_ding.load(Ordering::Relaxed);

    let mapping = backend
        .state
        .mapping
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let pack_name = mapping.as_ref().map(|mapping| mapping.pack_name.clone());
    let effect = mapping.as_ref().and_then(|mapping| {
        press_effect(
            event,
            mapping,
            modifier_sounds,
            key_up_sounds,
            key_up_fallback,
            return_ding,
        )
    });
    drop(mapping);

    match effect {
        None => {}
        Some(PlayEffect::File(path)) => {
            if let Err(error) = backend.audio.play(&path) {
                eprintln!("audio error: {error}");
            }

            backend
                .stats
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .bump_keystroke(pack_name.as_deref());
        }
        Some(PlayEffect::Ding) => {
            if let Some(ding) = backend.synthesized_ding()
                && let Err(error) = backend.audio.play_decoded(ding)
            {
                eprintln!("audio error: {error}");
            }

            backend
                .stats
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .bump_ding();
        }
    }
}

fn default_stats_path() -> PathBuf {
    dirs::data_dir()
        .map(|directory| directory.join("udu").join("stats.json"))
        .unwrap_or_else(|| PathBuf::from("stats.json"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlayEffect {
    File(PathBuf),
    Ding,
}

const MODIFIER_CODES: [u16; 8] = [29, 97, 42, 54, 56, 100, 125, 126];
const ENTER_CODES: [u16; 2] = [28, 96];

fn press_effect(
    event: KeyEvent,
    mapping: &mapping::Mapping,
    modifier_sounds: bool,
    key_up_sounds: bool,
    key_up_fallback: bool,
    return_ding: bool,
) -> Option<PlayEffect> {
    if MODIFIER_CODES.contains(&event.code) && !modifier_sounds {
        return None;
    }

    match event.kind {
        KeyEventKind::Press => {
            if return_ding && ENTER_CODES.contains(&event.code) {
                return Some(PlayEffect::Ding);
            }
            mapping
                .lookup_down(event.code)
                .map(Path::to_path_buf)
                .map(PlayEffect::File)
        }
        KeyEventKind::Release => {
            if !key_up_sounds {
                return None;
            }

            mapping
                .lookup_up(event.code)
                .or_else(|| {
                    key_up_fallback
                        .then(|| mapping.lookup_down(event.code))
                        .flatten()
                })
                .map(Path::to_path_buf)
                .map(PlayEffect::File)
        }
    }
}

impl BackendInner {
    fn synthesized_ding(&self) -> Option<Arc<audio::DecodedSound>> {
        let mut cache = self
            .ding
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let sound = cache.get_or_insert_with(|| Arc::new(audio::synthesized_ding()));
        Some(Arc::clone(sound))
    }
}

#[cfg(test)]
mod effect_tests {
    use super::{PlayEffect, fire, press_effect};
    use crate::backend::capture::{KeyEvent, KeyEventKind};
    use crate::backend::mapping::Mapping;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-effect-{name}-{}", std::process::id()))
    }

    fn load_mapping(name: &str, defines: &str) -> Mapping {
        let dir = test_dir(name);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("config.json"), defines).expect("config");
        for file in ["a.wav", "a-up.wav", "enter.wav"] {
            fs::write(dir.join(file), b"audio").expect("audio");
        }
        Mapping::load(&dir).expect("load")
    }

    struct RecordingAudio {
        played: std::sync::Mutex<Vec<PathBuf>>,
        dings: std::sync::atomic::AtomicUsize,
    }

    impl RecordingAudio {
        fn new() -> Self {
            Self {
                played: std::sync::Mutex::new(Vec::new()),
                dings: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl crate::backend::audio::AudioControl for RecordingAudio {
        fn play(&self, path: &Path) -> Result<(), crate::backend::audio::AudioError> {
            self.played
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(path.to_path_buf());
            Ok(())
        }

        fn play_decoded(
            &self,
            _sound: std::sync::Arc<crate::backend::audio::DecodedSound>,
        ) -> Result<(), crate::backend::audio::AudioError> {
            self.dings
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        fn set_master_volume(&self, _volume: f32) {}

        fn volume(&self) -> f32 {
            10.0
        }

        fn stream_failed(&self) -> bool {
            false
        }

        fn set_output_device(
            &self,
            _name: Option<&str>,
        ) -> Result<(), crate::backend::audio::AudioError> {
            Ok(())
        }

        fn output_device(&self) -> Option<String> {
            None
        }

        fn set_tone(&self, _pan: f32, _distance: f32) {}

        fn tone(&self) -> crate::backend::audio::TonePad {
            crate::backend::audio::TonePad::default()
        }

        fn set_variation(&self, _pitch: f32, _velocity: f32) {}
    }

    #[test]
    fn fire_plays_file_effects_and_counts_stats() {
        use crate::backend::stats::StatsStore;
        use crate::backend::{BackendInner, BackendState};
        use crate::config::AppConfig;
        use std::sync::atomic::{AtomicBool, AtomicU32};
        use std::sync::{Arc, Mutex, mpsc};

        let dir = test_dir("fire");
        fs::create_dir_all(&dir).expect("create dir");
        for file in ["config.json", "a.wav", "a-up.wav"] {
            fs::write(dir.join(file), b"audio").expect("audio");
        }
        fs::write(
            dir.join("config.json"),
            r#"{"defines":{"30":"a.wav","30-up":"a-up.wav"}}"#,
        )
        .expect("config");

        let mapping = load_mapping("fire", r#"{"defines":{"30":"a.wav","30-up":"a-up.wav"}}"#);
        let expected_pack = mapping.pack_name.clone();
        let recorder = Arc::new(RecordingAudio::new());
        let audio: Arc<dyn crate::backend::audio::AudioControl> = recorder.clone();
        let (tx, _rx) = mpsc::channel();

        let backend = Arc::new(BackendInner {
            audio,
            state: BackendState {
                mapping: Mutex::new(Some(mapping)),
                config: Mutex::new(AppConfig::default()),
            },
            config_path: dir.join("config.json"),
            desired_device: Mutex::new(None),
            events: tx,
            connected: AtomicBool::new(false),
            modifier_sounds: AtomicBool::new(true),
            key_up_sounds: AtomicBool::new(true),
            key_up_fallback: AtomicBool::new(true),
            pitch_variation: AtomicU32::new(crate::config::DEFAULT_PITCH_VARIATION.to_bits()),
            velocity_variation: AtomicU32::new(crate::config::DEFAULT_VELOCITY_VARIATION.to_bits()),
            return_ding: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
            ding: Mutex::new(None),
            stats: Mutex::new(StatsStore::load_or_default(dir.join("stats.json"))),
        });

        fire(
            &backend,
            KeyEvent {
                code: 30,
                kind: KeyEventKind::Press,
            },
        );

        let played = recorder
            .played
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        assert_eq!(played, vec![dir.join("a.wav")]);

        let stats = backend
            .stats
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot();
        assert_eq!(stats.keystrokes, 1);
        assert_eq!(stats.per_switch.get(&expected_pack), Some(&1));

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn muted_modifiers_are_silent_on_press_and_release() {
        let mapping = load_mapping("mods", r#"{"defines":{"29":"a.wav","29-up":"a-up.wav"}}"#);
        let press = KeyEvent {
            code: 29,
            kind: KeyEventKind::Press,
        };

        assert_eq!(
            press_effect(press, &mapping, false, true, true, false),
            None
        );

        let release = KeyEvent {
            code: 29,
            kind: KeyEventKind::Release,
        };
        assert_eq!(
            press_effect(release, &mapping, false, true, true, false),
            None
        );
    }

    #[test]
    fn unmuted_modifiers_play_like_any_key() {
        let mapping = load_mapping("mods-on", r#"{"defines":{"29":"a.wav"}}"#);
        let press = KeyEvent {
            code: 29,
            kind: KeyEventKind::Press,
        };

        assert!(matches!(
            press_effect(press, &mapping, true, true, true, false),
            Some(PlayEffect::File(_))
        ));
    }

    #[test]
    fn return_key_plays_the_ding_when_enabled() {
        let mapping = load_mapping("ding", r#"{"defines":{"28":"enter.wav"}}"#);
        let press = KeyEvent {
            code: 28,
            kind: KeyEventKind::Press,
        };

        assert_eq!(
            press_effect(press, &mapping, true, true, true, true),
            Some(PlayEffect::Ding)
        );
        assert!(matches!(
            press_effect(press, &mapping, true, true, true, false),
            Some(PlayEffect::File(_))
        ));
    }

    #[test]
    fn releases_use_the_up_sound_when_present() {
        let mapping = load_mapping("up", r#"{"defines":{"30":"a.wav","30-up":"a-up.wav"}}"#);
        let release = KeyEvent {
            code: 30,
            kind: KeyEventKind::Release,
        };

        let effect = press_effect(release, &mapping, true, true, true, true);
        assert!(
            matches!(effect, Some(PlayEffect::File(path)) if path.file_name().unwrap() == "a-up.wav")
        );
    }

    #[test]
    fn releases_reuse_the_down_sound_when_fallback_is_enabled() {
        let mapping = load_mapping("fallback", r#"{"defines":{"30":"a.wav"}}"#);
        let release = KeyEvent {
            code: 30,
            kind: KeyEventKind::Release,
        };

        let effect = press_effect(release, &mapping, true, true, true, false);

        assert!(
            matches!(effect, Some(PlayEffect::File(path)) if path.file_name().unwrap() == "a.wav")
        );
    }

    #[test]
    fn releases_stay_silent_when_fallback_is_disabled() {
        let mapping = load_mapping("fallback-off", r#"{"defines":{"30":"a.wav"}}"#);
        let release = KeyEvent {
            code: 30,
            kind: KeyEventKind::Release,
        };

        assert_eq!(
            press_effect(release, &mapping, true, true, false, false),
            None
        );
        assert_eq!(
            press_effect(release, &mapping, true, false, true, false),
            None
        );
    }
}

#[cfg(test)]
mod tests {
    use super::EngineEvent;
    #[test]
    fn engine_events_compare_by_value() {
        assert_eq!(EngineEvent::DeviceChanged, EngineEvent::DeviceChanged);
    }
}

#[cfg(test)]
mod engine_tests {
    use super::{
        BackendInner, BackendState, EngineEvent, MUTE_POLL_MS, RECONNECT_BASE_MS, RECONNECT_MAX_MS,
        run_engine,
    };
    use crate::backend::audio::{AudioControl, AudioError, DecodedSound, TonePad};
    use crate::backend::capture::{CaptureError, KeyEvent, KeyEventSource};
    use crate::backend::stats::StatsStore;
    use crate::config::AppConfig;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct NoopAudio;

    impl AudioControl for NoopAudio {
        fn play(&self, _path: &Path) -> Result<(), AudioError> {
            Ok(())
        }

        fn play_decoded(&self, _sound: Arc<DecodedSound>) -> Result<(), AudioError> {
            Ok(())
        }

        fn set_master_volume(&self, _volume: f32) {}

        fn volume(&self) -> f32 {
            0.0
        }

        fn stream_failed(&self) -> bool {
            false
        }

        fn set_output_device(&self, _name: Option<&str>) -> Result<(), AudioError> {
            Ok(())
        }

        fn output_device(&self) -> Option<String> {
            None
        }

        fn set_tone(&self, _pan: f32, _distance: f32) {}

        fn tone(&self) -> TonePad {
            TonePad::default()
        }

        fn set_variation(&self, _pitch: f32, _velocity: f32) {}
    }

    struct FakeSource {
        responses: VecDeque<Result<Option<KeyEvent>, CaptureError>>,
        on_first_call: Option<Box<dyn FnOnce()>>,
    }

    impl FakeSource {
        fn new(responses: Vec<Result<Option<KeyEvent>, CaptureError>>) -> Self {
            Self {
                responses: responses.into(),
                on_first_call: None,
            }
        }

        fn with_side_effect(
            responses: Vec<Result<Option<KeyEvent>, CaptureError>>,
            effect: impl FnOnce() + 'static,
        ) -> Self {
            Self {
                responses: responses.into(),
                on_first_call: Some(Box::new(effect)),
            }
        }
    }

    impl KeyEventSource for FakeSource {
        fn next_key_event(&mut self) -> Result<Option<KeyEvent>, CaptureError> {
            if let Some(effect) = self.on_first_call.take() {
                effect();
            }

            self.responses.pop_front().unwrap_or(Ok(None))
        }
    }

    fn device_gone() -> CaptureError {
        CaptureError::DeviceGone {
            path: PathBuf::from("/dev/input/fake"),
        }
    }

    fn test_backend(device: Option<&str>, enabled: bool) -> Arc<BackendInner> {
        let (tx, _rx) = mpsc::channel();

        Arc::new(BackendInner {
            audio: Arc::new(NoopAudio),
            state: BackendState {
                mapping: Mutex::new(None),
                config: Mutex::new(AppConfig::default()),
            },
            config_path: PathBuf::new(),
            desired_device: Mutex::new(device.map(str::to_string)),
            events: tx,
            connected: AtomicBool::new(false),
            modifier_sounds: AtomicBool::new(true),
            key_up_sounds: AtomicBool::new(true),
            key_up_fallback: AtomicBool::new(true),
            pitch_variation: AtomicU32::new(crate::config::DEFAULT_PITCH_VARIATION.to_bits()),
            velocity_variation: AtomicU32::new(crate::config::DEFAULT_VELOCITY_VARIATION.to_bits()),
            return_ding: AtomicBool::new(false),
            enabled: AtomicBool::new(enabled),
            ding: Mutex::new(None),
            stats: Mutex::new(StatsStore::load_or_default(PathBuf::new())),
        })
    }

    #[test]
    fn backoff_doubles_and_caps_at_ten_seconds() {
        let backend = test_backend(Some("missing"), true);
        let (events_tx, events_rx) = mpsc::channel();

        let recorded = Arc::new(Mutex::new(Vec::new()));
        let recorded_sleeps = Arc::clone(&recorded);
        let mut sleep_calls = 0u32;
        let sleep_fake = move |duration: Duration| {
            recorded_sleeps
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(duration);
            sleep_calls += 1;

            if sleep_calls >= 7 {
                let _ = events_tx.send(EngineEvent::Shutdown);
            }
        };

        run_engine(
            &backend,
            events_rx,
            |_name: &str| -> Option<FakeSource> { None },
            sleep_fake,
            || {},
        );

        let expected: Vec<Duration> = std::iter::successors(Some(RECONNECT_BASE_MS), |&backoff| {
            Some((backoff * 2).min(RECONNECT_MAX_MS))
        })
        .take(7)
        .map(Duration::from_millis)
        .collect();

        let recorded = recorded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(*recorded, expected);
    }

    #[test]
    fn backoff_resets_to_base_after_a_successful_reopen() {
        let backend = test_backend(Some("device"), true);
        let (events_tx, events_rx) = mpsc::channel();

        let recorded = Arc::new(Mutex::new(Vec::new()));
        let recorded_sleeps = Arc::clone(&recorded);
        let sleep_fake = move |duration: Duration| {
            recorded_sleeps
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(duration);
        };

        let open_calls = Arc::new(Mutex::new(0u32));
        let open_calls_counter = Arc::clone(&open_calls);
        let events_tx_for_open = events_tx.clone();
        let open_fake = move |_name: &str| -> Option<FakeSource> {
            let mut calls = open_calls_counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *calls += 1;
            let call_number = *calls;
            drop(calls);

            match call_number {
                1 | 2 => None,
                3 => Some(FakeSource::new(vec![Err(device_gone())])),
                _ => {
                    let _ = events_tx_for_open.send(EngineEvent::Shutdown);
                    None
                }
            }
        };

        run_engine(&backend, events_rx, open_fake, sleep_fake, || {});

        let recorded = recorded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(
            *recorded,
            vec![
                Duration::from_millis(RECONNECT_BASE_MS),
                Duration::from_millis(RECONNECT_BASE_MS * 2),
                Duration::from_millis(RECONNECT_BASE_MS),
            ]
        );
    }

    #[test]
    fn device_changed_forces_a_reopen_while_already_connected() {
        let backend = test_backend(Some("device"), true);
        let (events_tx, events_rx) = mpsc::channel();

        let open_calls = Arc::new(Mutex::new(0u32));
        let open_calls_counter = Arc::clone(&open_calls);
        let events_tx_for_source = events_tx.clone();
        let events_tx_for_open = events_tx.clone();
        let open_fake = move |_name: &str| -> Option<FakeSource> {
            let mut calls = open_calls_counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *calls += 1;
            let call_number = *calls;
            drop(calls);

            if call_number == 1 {
                let sender = events_tx_for_source.clone();
                return Some(FakeSource::with_side_effect(vec![Ok(None)], move || {
                    let _ = sender.send(EngineEvent::DeviceChanged);
                }));
            }

            let _ = events_tx_for_open.send(EngineEvent::Shutdown);
            Some(FakeSource::new(vec![Ok(None)]))
        };

        run_engine(
            &backend,
            events_rx,
            open_fake,
            |_duration: Duration| {},
            || {},
        );

        let calls = *open_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(calls, 2);
    }

    #[test]
    fn device_gone_clears_connected_and_retries_without_exiting() {
        let backend = test_backend(Some("device"), true);
        let (events_tx, events_rx) = mpsc::channel();

        let open_calls = Arc::new(Mutex::new(0u32));
        let open_calls_counter = Arc::clone(&open_calls);
        let open_fake = move |_name: &str| -> Option<FakeSource> {
            let mut calls = open_calls_counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *calls += 1;
            let call_number = *calls;
            drop(calls);

            match call_number {
                1 => Some(FakeSource::new(vec![Err(device_gone())])),
                _ => None,
            }
        };

        let connected_at_retry = Arc::new(Mutex::new(None));
        let connected_probe = Arc::clone(&connected_at_retry);
        let backend_probe = Arc::clone(&backend);
        let events_tx_for_sleep = events_tx.clone();
        let sleep_fake = move |_duration: Duration| {
            *connected_probe
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(backend_probe.connected.load(Ordering::Relaxed));
            let _ = events_tx_for_sleep.send(EngineEvent::Shutdown);
        };

        run_engine(&backend, events_rx, open_fake, sleep_fake, || {});

        let calls = *open_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(calls, 2, "the engine must retry opening after DeviceGone");

        let observed = connected_at_retry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expect("sleep must run, proving the thread kept going");
        assert!(!observed, "DeviceGone must clear connected before retrying");
    }

    #[test]
    fn shutdown_exits_the_loop_before_any_reconnect_attempt() {
        let backend = test_backend(Some("device"), true);
        let (events_tx, events_rx) = mpsc::channel();
        events_tx
            .send(EngineEvent::Shutdown)
            .expect("send shutdown");

        let open_calls = Arc::new(Mutex::new(0u32));
        let open_calls_counter = Arc::clone(&open_calls);
        let open_fake = move |_name: &str| -> Option<FakeSource> {
            *open_calls_counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
            None
        };

        run_engine(
            &backend,
            events_rx,
            open_fake,
            |_duration: Duration| {},
            || {},
        );

        let calls = *open_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(calls, 0);
    }

    #[test]
    fn muting_releases_the_capture_and_skips_reconnect_attempts() {
        let backend = test_backend(Some("device"), true);
        let backend_for_effect = Arc::clone(&backend);
        let (events_tx, events_rx) = mpsc::channel();

        let open_calls = Arc::new(Mutex::new(0u32));
        let open_calls_counter = Arc::clone(&open_calls);
        let open_fake = move |_name: &str| -> Option<FakeSource> {
            let mut calls = open_calls_counter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *calls += 1;
            let call_number = *calls;
            drop(calls);

            if call_number == 1 {
                let backend = Arc::clone(&backend_for_effect);
                return Some(FakeSource::with_side_effect(vec![Ok(None)], move || {
                    backend.enabled.store(false, Ordering::Relaxed);
                }));
            }

            None
        };

        let mute_sleep_seen = Arc::new(Mutex::new(false));
        let mute_sleep_probe = Arc::clone(&mute_sleep_seen);
        let events_tx_for_sleep = events_tx.clone();
        let sleep_fake = move |duration: Duration| {
            if duration == Duration::from_millis(MUTE_POLL_MS) {
                *mute_sleep_probe
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
                let _ = events_tx_for_sleep.send(EngineEvent::Shutdown);
            }
        };

        run_engine(&backend, events_rx, open_fake, sleep_fake, || {});

        assert!(
            *mute_sleep_seen
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            "muting must take the mute-poll path"
        );
        assert!(!backend.connected.load(Ordering::Relaxed));
        assert_eq!(
            *open_calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            1,
            "a muted engine must not try to reopen the source"
        );
    }
}
