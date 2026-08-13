use crate::backend::mapping::Mapping;
use crate::backend::stats::Stats;
use crate::backend::{BackendInner, BackendStatus, EngineEvent};
use crate::config::{AppConfig, clamp_volume, save_config};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SOCKET_NAME: &str = "udu.sock";
const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<BackendStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported: Option<String>,
}

impl Response {
    fn ok(status: BackendStatus) -> Self {
        Self {
            ok: true,
            error: None,
            status: Some(status),
            stats: None,
            exported: None,
        }
    }

    fn error(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(error.into()),
            status: None,
            stats: None,
            exported: None,
        }
    }
}

pub fn socket_path() -> Result<PathBuf, String> {
    let runtime_directory = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| String::from("XDG_RUNTIME_DIR is not available for the control socket"))?;

    Ok(runtime_directory.join(SOCKET_NAME))
}

pub fn serve(socket_path: &Path, backend: &BackendInner) -> Result<(), io::Error> {
    let listener = bind(socket_path)?;

    std::thread::scope(|scope| {
        for incoming in listener.incoming() {
            let stream = match incoming {
                Ok(stream) => stream,
                Err(_) => continue,
            };

            if !peer_uid_matches(&stream) {
                continue;
            }

            let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));
            let _ = stream.set_write_timeout(Some(REQUEST_TIMEOUT));

            scope.spawn(|| handle_connection(stream, backend));
        }
    });

    Ok(())
}

fn peer_uid_matches(stream: &UnixStream) -> bool {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut credentials).cast::<libc::c_void>(),
            &mut length,
        )
    };

    result == 0 && credentials.uid == unsafe { libc::geteuid() }
}

fn handle_connection(stream: UnixStream, backend: &BackendInner) {
    let Ok(writer) = stream.try_clone() else {
        return;
    };
    let mut writer = writer;
    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        let bytes_read = match (&mut reader).take(MAX_REQUEST_BYTES).read_line(&mut line) {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(_) => break,
        };

        if bytes_read as u64 == MAX_REQUEST_BYTES && !line.ends_with('\n') {
            break;
        }

        let response = handle_request(backend, &line);
        let Ok(payload) = serde_json::to_string(&response) else {
            break;
        };
        if writeln!(writer, "{payload}").is_err() {
            break;
        }
        let _ = writer.flush();
    }
}

pub fn handle_request(backend: &BackendInner, line: &str) -> Response {
    let request: Request = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => return Response::error(format!("invalid request: {error}")),
    };

    apply(request, backend)
}

fn apply(request: Request, backend: &BackendInner) -> Response {
    match request.cmd.as_str() {
        "set_soundpack" => apply_soundpack(request, backend),
        "set_volume" => apply_volume(request, backend),
        "set_device" => apply_device(request, backend),
        "set_modifier_sounds" => apply_flag(request, backend, "modifier_sounds"),
        "set_key_up_sounds" => apply_flag(request, backend, "key_up_sounds"),
        "set_key_up_fallback" => apply_flag(request, backend, "key_up_fallback"),
        "set_pitch_variation" => apply_variation(request, backend, VariationTarget::Pitch),
        "set_velocity_variation" => apply_variation(request, backend, VariationTarget::Velocity),
        "set_return_ding" => apply_flag(request, backend, "return_ding"),
        "set_output_device" => apply_output_device(request, backend),
        "set_enabled" => apply_enabled(request, backend),
        "play_sample" => apply_play_sample(request, backend),
        "play_ding" => apply_play_ding(request, backend),
        "get_stats" => apply_get_stats(request, backend),
        "export_stats" => apply_export_stats(request, backend),
        "reset_stats" => apply_reset_stats(request, backend),
        "set_tone_pan" => apply_tone(request, backend, true),
        "set_tone_distance" => apply_tone(request, backend, false),
        "status" => Response::ok(current_status(backend)),
        unknown => Response::error(format!("unknown command '{unknown}'")),
    }
}

fn apply_soundpack(request: Request, backend: &BackendInner) -> Response {
    let Some(path) = request.path else {
        return Response::error("set_soundpack requires a 'path'");
    };

    let path = match validated_path(backend, &path) {
        Ok(path) => path,
        Err(error) => return Response::error(format!("invalid soundpack path: {error}")),
    };

    let mapping = match Mapping::load(&path) {
        Ok(mapping) => mapping,
        Err(error) => return Response::error(error.to_string()),
    };

    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    config.selected_soundpack = Some(path);
    if let Err(error) = persist(backend, &config) {
        return Response::error(error);
    }
    *backend
        .state
        .mapping
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(mapping);

    Response::ok(current_status(backend))
}

fn apply_volume(request: Request, backend: &BackendInner) -> Response {
    let Some(value) = request.value else {
        return Response::error("set_volume requires a 'value'");
    };

    let volume = clamp_volume(value);
    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = config.volume;
    config.volume = volume;
    if let Err(error) = persist(backend, &config) {
        config.volume = previous;
        return Response::error(error);
    }
    drop(config);

    backend.audio.set_master_volume(volume);

    Response::ok(current_status(backend))
}

fn apply_device(request: Request, backend: &BackendInner) -> Response {
    let Some(name) = request.name else {
        return Response::error("set_device requires a 'name'");
    };

    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous_device = config.device_name.clone();
    config.device_name = Some(name.clone());
    if let Err(error) = persist(backend, &config) {
        config.device_name = previous_device;
        return Response::error(error);
    }
    drop(config);

    *backend
        .desired_device
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(name);
    let _ = backend.events.send(EngineEvent::DeviceChanged);

    Response::ok(current_status(backend))
}

fn apply_enabled(request: Request, backend: &BackendInner) -> Response {
    let Some(value) = request.value else {
        return Response::error("set_enabled requires a 'value'");
    };
    backend
        .enabled
        .store(value != 0.0, std::sync::atomic::Ordering::Relaxed);

    Response::ok(current_status(backend))
}

fn apply_play_sample(request: Request, backend: &BackendInner) -> Response {
    let Some(path) = request.path else {
        return Response::error("play_sample requires a 'path'");
    };

    let path = match validated_path(backend, &path) {
        Ok(path) => path,
        Err(error) => return Response::error(format!("invalid sample path: {error}")),
    };

    match backend.audio.play(&path) {
        Ok(()) => Response::ok(current_status(backend)),
        Err(error) => Response::error(error.to_string()),
    }
}

fn validated_path(backend: &BackendInner, path: &Path) -> Result<PathBuf, String> {
    let roots = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .soundpack_roots
        .clone();

    resolve_within_roots(path, &roots)
}

fn resolve_within_roots(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|_| String::from("path does not exist or is not accessible"))?;

    let is_allowed = roots.iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|canonical_root| canonical.starts_with(canonical_root))
    });

    if !is_allowed {
        return Err(String::from(
            "path is outside the configured soundpack roots",
        ));
    }

    Ok(canonical)
}

fn apply_play_ding(_request: Request, backend: &BackendInner) -> Response {
    let ding = backend.synthesized_ding();

    match ding.and_then(|ding| backend.audio.play_decoded(ding).ok()) {
        Some(()) => Response::ok(current_status(backend)),
        None => Response::error("could not play the feedback ding"),
    }
}

fn apply_get_stats(_request: Request, backend: &BackendInner) -> Response {
    let stats = backend
        .stats
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot();

    Response {
        ok: true,
        error: None,
        status: Some(current_status(backend)),
        stats: Some(stats),
        exported: None,
    }
}

fn apply_export_stats(_request: Request, backend: &BackendInner) -> Response {
    let exported = backend
        .stats
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .export_markdown();

    Response {
        ok: true,
        error: None,
        status: Some(current_status(backend)),
        stats: None,
        exported: Some(exported),
    }
}

fn apply_reset_stats(_request: Request, backend: &BackendInner) -> Response {
    backend
        .stats
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .reset();

    Response::ok(current_status(backend))
}

fn apply_tone(request: Request, backend: &BackendInner, is_pan: bool) -> Response {
    let Some(value) = request.value else {
        return Response::error("tone command requires a 'value'");
    };

    let current = backend.audio.tone();
    let (pan, distance) = if is_pan {
        (value.clamp(-1.0, 1.0), current.distance)
    } else {
        (current.pan, value.clamp(0.0, 1.0))
    };

    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = config.clone().tone_pan;
    let previous_distance = config.clone().tone_distance;
    config.tone_pan = pan;
    config.tone_distance = distance;
    if let Err(error) = persist(backend, &config) {
        config.tone_pan = previous;
        config.tone_distance = previous_distance;
        return Response::error(error);
    }
    drop(config);

    backend.audio.set_tone(pan, distance);

    Response::ok(current_status(backend))
}

fn apply_flag(request: Request, backend: &BackendInner, label: &str) -> Response {
    let Some(value) = request.value else {
        return Response::error(format!("{label} requires a 'value'"));
    };
    let enabled = value != 0.0;

    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (previous, target) = match label {
        "modifier_sounds" => (config.modifier_sounds, &backend.modifier_sounds),
        "key_up_sounds" => (config.key_up_sounds, &backend.key_up_sounds),
        "key_up_fallback" => (config.key_up_fallback, &backend.key_up_fallback),
        "return_ding" => (config.return_ding, &backend.return_ding),
        _ => unreachable!("only known toggles"),
    };
    match label {
        "modifier_sounds" => config.modifier_sounds = enabled,
        "key_up_sounds" => config.key_up_sounds = enabled,
        "key_up_fallback" => config.key_up_fallback = enabled,
        _ => config.return_ding = enabled,
    }
    if let Err(error) = persist(backend, &config) {
        match label {
            "modifier_sounds" => config.modifier_sounds = previous,
            "key_up_sounds" => config.key_up_sounds = previous,
            "key_up_fallback" => config.key_up_fallback = previous,
            _ => config.return_ding = previous,
        }
        return Response::error(error);
    }
    drop(config);

    target.store(enabled, std::sync::atomic::Ordering::Relaxed);

    Response::ok(current_status(backend))
}

enum VariationTarget {
    Pitch,
    Velocity,
}

fn apply_variation(request: Request, backend: &BackendInner, target: VariationTarget) -> Response {
    let Some(value) = request.value else {
        return Response::error("variation command requires a 'value'");
    };

    let value = value.clamp(0.0, crate::config::MAX_VARIATION);
    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = match target {
        VariationTarget::Pitch => config.pitch_variation,
        VariationTarget::Velocity => config.velocity_variation,
    };
    match target {
        VariationTarget::Pitch => config.pitch_variation = value,
        VariationTarget::Velocity => config.velocity_variation = value,
    }
    if let Err(error) = persist(backend, &config) {
        match target {
            VariationTarget::Pitch => config.pitch_variation = previous,
            VariationTarget::Velocity => config.velocity_variation = previous,
        }
        return Response::error(error);
    }
    drop(config);

    if matches!(target, VariationTarget::Pitch) {
        backend
            .pitch_variation
            .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    } else {
        backend
            .velocity_variation
            .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
    backend.audio.set_variation(
        f32::from_bits(
            backend
                .pitch_variation
                .load(std::sync::atomic::Ordering::Relaxed),
        ),
        f32::from_bits(
            backend
                .velocity_variation
                .load(std::sync::atomic::Ordering::Relaxed),
        ),
    );

    Response::ok(current_status(backend))
}

fn apply_output_device(request: Request, backend: &BackendInner) -> Response {
    let name = request.name.clone();

    if let Err(error) = backend.audio.set_output_device(name.as_deref()) {
        return Response::error(error.to_string());
    }

    let mut config = backend
        .state
        .config
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = config.output_device.clone();
    config.output_device = name;
    if let Err(error) = persist(backend, &config) {
        config.output_device = previous.clone();
        let _ = backend.audio.set_output_device(previous.as_deref());
        return Response::error(error);
    }

    Response::ok(current_status(backend))
}

fn persist(backend: &BackendInner, config: &AppConfig) -> Result<(), String> {
    save_config(&backend.config_path, config)
        .map_err(|error| format!("could not persist configuration: {error}"))
}

fn current_status(backend: &BackendInner) -> BackendStatus {
    let mapping = backend
        .state
        .mapping
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let device = backend
        .desired_device
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    BackendStatus {
        soundpack: mapping.as_ref().map(|mapping| mapping.pack_name.clone()),
        volume: backend.audio.volume(),
        device: device.clone(),
        device_connected: backend.connected.load(std::sync::atomic::Ordering::Relaxed),
        stream_failed: backend.audio.stream_failed(),
        output_device: backend.audio.output_device(),
        tone_pan: backend.audio.tone().pan,
        tone_distance: backend.audio.tone().distance,
        enabled: backend.enabled.load(std::sync::atomic::Ordering::Relaxed),
        modifier_sounds: backend
            .modifier_sounds
            .load(std::sync::atomic::Ordering::Relaxed),
        key_up_sounds: backend
            .key_up_sounds
            .load(std::sync::atomic::Ordering::Relaxed),
        key_up_fallback: backend
            .key_up_fallback
            .load(std::sync::atomic::Ordering::Relaxed),
        pitch_variation: f32::from_bits(
            backend
                .pitch_variation
                .load(std::sync::atomic::Ordering::Relaxed),
        ),
        velocity_variation: f32::from_bits(
            backend
                .velocity_variation
                .load(std::sync::atomic::Ordering::Relaxed),
        ),
        return_ding: backend
            .return_ding
            .load(std::sync::atomic::Ordering::Relaxed),
    }
}

fn bind(socket_path: &Path) -> io::Result<UnixListener> {
    match UnixListener::bind(socket_path) {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            if !is_stale_socket(socket_path) {
                return Err(error);
            }
            let _ = fs::remove_file(socket_path);
            UnixListener::bind(socket_path)
        }
        Err(error) => Err(error),
    }
}

fn is_stale_socket(socket_path: &Path) -> bool {
    match UnixStream::connect(socket_path) {
        Ok(_) => false,
        Err(error) => matches!(
            error.kind(),
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, apply, handle_request};
    use crate::backend::{BackendInner, BackendStatus, EngineEvent};
    use crate::config::{AppConfig, load_config};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn write_test_pack(directory: &std::path::Path) {
        std::fs::create_dir_all(directory).expect("create pack directory");
        std::fs::write(
            directory.join("config.json"),
            r#"{"defines":{"30":"a.wav"}}"#,
        )
        .expect("write pack config");
        std::fs::write(directory.join("a.wav"), b"audio").expect("write pack audio");
    }

    struct SilentAudio {
        output_device: Mutex<Option<String>>,
        tone: Mutex<crate::backend::audio::TonePad>,
    }

    impl SilentAudio {
        fn new() -> Self {
            Self {
                output_device: Mutex::new(None),
                tone: Mutex::new(crate::backend::audio::TonePad::default()),
            }
        }
    }

    impl crate::backend::audio::AudioControl for SilentAudio {
        fn play(&self, _path: &std::path::Path) -> Result<(), crate::backend::audio::AudioError> {
            Ok(())
        }

        fn play_decoded(
            &self,
            _sound: std::sync::Arc<crate::backend::audio::DecodedSound>,
        ) -> Result<(), crate::backend::audio::AudioError> {
            Ok(())
        }

        fn set_master_volume(&self, _volume: f32) {}
        fn volume(&self) -> f32 {
            1.0
        }
        fn stream_failed(&self) -> bool {
            false
        }

        fn set_output_device(
            &self,
            name: Option<&str>,
        ) -> Result<(), crate::backend::audio::AudioError> {
            *self
                .output_device
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = name.map(str::to_string);
            Ok(())
        }

        fn output_device(&self) -> Option<String> {
            self.output_device
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        fn set_tone(&self, pan: f32, distance: f32) {
            *self
                .tone
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                crate::backend::audio::TonePad { pan, distance };
        }

        fn tone(&self) -> crate::backend::audio::TonePad {
            *self
                .tone
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        fn set_variation(&self, _pitch: f32, _velocity: f32) {}
    }

    fn test_backend(
        _name: &str,
    ) -> (
        Arc<BackendInner>,
        std::path::PathBuf,
        mpsc::Receiver<EngineEvent>,
    ) {
        let root =
            std::env::temp_dir().join(format!("wayvibes-control-{_name}-{}", std::process::id()));
        let config_path = root.join("config.json");
        std::fs::create_dir_all(&root).expect("create temp directory");

        let (tx, rx) = mpsc::channel();
        let backend = BackendInner {
            audio: Arc::new(SilentAudio::new()),
            state: crate::backend::BackendState {
                mapping: Mutex::new(None),
                config: Mutex::new(AppConfig::default()),
            },
            config_path: config_path.clone(),
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
            stats: Mutex::new(crate::backend::stats::StatsStore::load_or_default(
                std::path::PathBuf::new(),
            )),
        };

        (Arc::new(backend), config_path, rx)
    }

    #[test]
    fn parses_valid_requests() {
        let request: Request =
            serde_json::from_str(r#"{"cmd":"set_volume","value":2.5}"#).expect("parse");

        assert_eq!(request.cmd, "set_volume");
        assert_eq!(request.value, Some(2.5));
    }

    #[test]
    fn malformed_lines_return_an_error_response() {
        let (backend, _root, _rx) = test_backend("malformed");

        let response = handle_request(&backend, "not json");

        assert!(!response.ok);
        assert!(response.error.unwrap().contains("invalid request"));
    }

    #[test]
    fn unknown_commands_return_an_error_response() {
        let (backend, _root, _rx) = test_backend("unknown");

        let response = handle_request(&backend, r#"{"cmd":"explode"}"#);

        assert!(!response.ok);
        assert!(response.error.unwrap().contains("unknown command"));
    }

    #[test]
    fn set_volume_applies_and_persists() {
        let (backend, config_path, _rx) = test_backend("volume");

        let response = handle_request(&backend, r#"{"cmd":"set_volume","value":4.0}"#);

        assert!(response.ok);
        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.volume, 4.0);
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn set_volume_clamps_and_requests_missing_value() {
        let (backend, config_path, _rx) = test_backend("clamp");

        let response = handle_request(&backend, r#"{"cmd":"set_volume","value":250.0}"#);
        assert!(response.ok);
        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.volume, 100.0);

        let missing = handle_request(&backend, r#"{"cmd":"set_volume"}"#);
        assert!(!missing.ok);

        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn set_device_applies_persists_and_notifies_the_engine() {
        let (backend, config_path, rx) = test_backend("device");

        let response = handle_request(&backend, r#"{"cmd":"set_device","name":"USB Keyboard"}"#);

        assert!(response.ok);
        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.device_name.as_deref(), Some("USB Keyboard"));
        assert!(matches!(rx.try_recv(), Ok(EngineEvent::DeviceChanged)));
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn status_reports_live_state() {
        let (backend, config_path, _rx) = test_backend("status");

        let response = handle_request(&backend, r#"{"cmd":"status"}"#);

        assert!(response.ok);
        let status = response.status.expect("status field");
        assert_eq!(status.volume, 1.0);
        assert_eq!(status.soundpack, None);
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn soundpack_mapping_errors_surface_cleanly() {
        let (backend, config_path, _rx) = test_backend("soundpack");

        let response = handle_request(
            &backend,
            r#"{"cmd":"set_soundpack","path":"/nonexistent/pack"}"#,
        );

        assert!(!response.ok);
        assert!(response.error.unwrap().contains("soundpack"));
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn response_serialization_round_trips() {
        let status = BackendStatus {
            soundpack: Some(String::from("Creams")),
            volume: 2.0,
            device: Some(String::from("kbd")),
            device_connected: true,
            stream_failed: false,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 1.0,
            enabled: true,
            modifier_sounds: true,
            key_up_sounds: true,
            key_up_fallback: true,
            pitch_variation: crate::config::DEFAULT_PITCH_VARIATION,
            velocity_variation: crate::config::DEFAULT_VELOCITY_VARIATION,
            return_ding: false,
        };
        let response = Response {
            ok: true,
            error: None,
            status: Some(status),
            stats: None,
            exported: None,
        };

        let payload = serde_json::to_string(&response).expect("serialize");
        let decoded: Response = serde_json::from_str(&payload).expect("deserialize");

        assert!(decoded.ok);
        assert_eq!(decoded.status.unwrap().soundpack.unwrap(), "Creams");
    }

    #[test]
    fn modifier_and_ding_toggles_persist_and_apply() {
        let (backend, config_path, _rx) = test_backend("toggles");

        let response = handle_request(&backend, r#"{"cmd":"set_modifier_sounds","value":0.0}"#);
        assert!(response.ok);
        assert!(!response.status.unwrap().modifier_sounds);

        let response = handle_request(&backend, r#"{"cmd":"set_return_ding","value":1.0}"#);
        assert!(response.ok);
        assert!(response.status.unwrap().return_ding);

        let persisted = load_config(&config_path).expect("load persisted config");
        assert!(!persisted.modifier_sounds);
        assert!(persisted.return_ding);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn output_device_applies_persists_and_restores_on_failure() {
        let (backend, config_path, _rx) = test_backend("output");

        let response = handle_request(
            &backend,
            r#"{"cmd":"set_output_device","name":"Fake Speakers"}"#,
        );

        assert!(response.ok);
        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.output_device.as_deref(), Some("Fake Speakers"));
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn mute_and_ding_commands_round_trip() {
        let (backend, config_path, _rx) = test_backend("cmds");

        let response = handle_request(&backend, r#"{"cmd":"set_enabled","value":0.0}"#);
        assert!(response.ok);
        assert!(!response.status.unwrap().enabled);

        let ding = handle_request(&backend, r#"{"cmd":"play_ding"}"#);
        assert!(ding.ok);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn variation_commands_persist_and_apply() {
        let (backend, config_path, _rx) = test_backend("variation");

        let response = handle_request(&backend, r#"{"cmd":"set_pitch_variation","value":0.2}"#);
        assert!(response.ok);
        assert_eq!(response.status.unwrap().pitch_variation, 0.2);

        let response = handle_request(&backend, r#"{"cmd":"set_velocity_variation","value":0.3}"#);
        assert!(response.ok);
        assert_eq!(response.status.unwrap().velocity_variation, 0.3);

        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.pitch_variation, 0.2);
        assert_eq!(persisted.velocity_variation, 0.3);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn tone_commands_persist_and_apply() {
        let (backend, config_path, _rx) = test_backend("tone");
        let backend = &backend;

        let response = handle_request(backend, r#"{"cmd":"set_tone_pan","value":1.0}"#);
        assert!(response.ok);
        assert_eq!(response.status.unwrap().tone_pan, 1.0);

        let response = handle_request(backend, r#"{"cmd":"set_tone_distance","value":0.5}"#);
        assert!(response.ok);
        assert_eq!(response.status.unwrap().tone_distance, 0.5);

        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(persisted.tone_pan, 1.0);
        assert_eq!(persisted.tone_distance, 0.5);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn stats_commands_snapshot_export_and_reset() {
        let (backend, config_path, _rx) = test_backend("stats-cmds");

        let snapshot = handle_request(&backend, r#"{"cmd":"get_stats"}"#);
        assert!(snapshot.ok);
        assert_eq!(snapshot.stats.unwrap().keystrokes, 0);

        let exported = handle_request(&backend, r#"{"cmd":"export_stats"}"#);
        assert!(exported.ok);
        assert!(exported.exported.unwrap().starts_with("# udu usage stats"));

        let reset = handle_request(&backend, r#"{"cmd":"reset_stats"}"#);
        assert!(reset.ok);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn toggles_require_a_value() {
        let (backend, config_path, _rx) = test_backend("toggle-missing");

        let response = handle_request(&backend, r#"{"cmd":"set_return_ding"}"#);
        assert!(!response.ok);
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn bind_rejects_a_live_second_listener_and_recovers_a_stale_socket() {
        let directory = std::env::temp_dir().join(format!("wayvibes-bind-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let live = directory.join("live.sock");
        let stale = directory.join("stale.sock");

        let first = crate::backend::control::UnixListener::bind(&live).expect("first bind");
        let error = super::bind(&live).expect_err("second bind must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        drop(first);

        let dropped = crate::backend::control::UnixListener::bind(&stale).expect("create stale");
        drop(dropped);
        let rebound = super::bind(&stale).expect("stale socket is replaced");
        drop(rebound);

        std::fs::remove_dir_all(directory).expect("remove temp directory");
    }

    #[test]
    fn apply_handles_the_status_command_and_unknown_commands() {
        let (backend, _root, _rx) = test_backend("exhaustive");
        let request = Request {
            cmd: String::from("status"),
            path: None,
            value: None,
            name: None,
        };

        let response = apply(request, &backend);
        assert!(response.ok);

        let unknown = Request {
            cmd: String::from("explode"),
            path: None,
            value: None,
            name: None,
        };
        assert!(!apply(unknown, &backend).ok);
    }

    #[test]
    fn oversized_request_without_a_newline_is_dropped_instead_of_hanging() {
        let (backend, config_path, _rx) = test_backend("oversized");
        let backend_ref = Arc::clone(&backend);
        let (server_stream, mut client_stream) = UnixStream::pair().expect("create socket pair");

        let handler =
            std::thread::spawn(move || super::handle_connection(server_stream, &backend_ref));

        let chunk = vec![b'a'; 8 * 1024];
        let target = 2 * super::MAX_REQUEST_BYTES as usize;
        let mut written = 0usize;
        while written < target {
            match client_stream.write(&chunk) {
                Ok(0) | Err(_) => break,
                Ok(sent) => written += sent,
            }
        }

        let mut response = Vec::new();
        let outcome = client_stream.read_to_end(&mut response);
        let dropped_cleanly = outcome.is_ok()
            || matches!(
                outcome.as_ref().err().map(std::io::Error::kind),
                Some(std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe)
            );
        assert!(dropped_cleanly, "unexpected read outcome: {outcome:?}");
        assert!(
            response.is_empty(),
            "an oversized request without a newline must not receive a response"
        );

        handler.join().expect("handler thread completes");
        std::fs::remove_dir_all(config_path.parent().unwrap()).expect("cleanup");
    }

    #[test]
    fn a_stalled_connection_does_not_block_other_clients() {
        let directory = std::env::temp_dir().join(format!(
            "wayvibes-control-stall-socket-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let socket = directory.join("udu.sock");
        let (backend, config_path, _rx) = test_backend("stall");

        let server_backend = Arc::clone(&backend);
        let server_socket = socket.clone();
        let server = std::thread::spawn(move || {
            let _ = super::serve(&server_socket, &server_backend);
        });
        std::thread::sleep(Duration::from_millis(150));

        let stalled = UnixStream::connect(&socket).expect("open the stalling connection");

        let mut second = UnixStream::connect(&socket).expect("open the second connection");
        second
            .write_all(b"{\"cmd\":\"status\"}\n")
            .expect("send a status request");
        second.flush().expect("flush the status request");

        let mut reader = BufReader::new(second);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("the second client must receive a prompt reply");
        assert!(line.contains("\"ok\":true"));

        drop(stalled);
        drop(server);
        let _ = std::fs::remove_dir_all(directory);
        let _ = std::fs::remove_dir_all(config_path.parent().unwrap());
    }

    #[test]
    fn resolve_within_roots_accepts_a_path_inside_a_configured_root() {
        let base =
            std::env::temp_dir().join(format!("wayvibes-roots-accept-{}", std::process::id()));
        let root = base.join("root");
        let pack = root.join("pack");
        write_test_pack(&pack);

        let resolved =
            super::resolve_within_roots(&pack, &[root]).expect("path inside a root is accepted");
        assert_eq!(resolved, pack.canonicalize().expect("canonicalize pack"));

        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn resolve_within_roots_rejects_a_path_outside_every_root() {
        let base =
            std::env::temp_dir().join(format!("wayvibes-roots-outside-{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside-pack");
        write_test_pack(&root.join("allowed"));
        write_test_pack(&outside);

        let error =
            super::resolve_within_roots(&outside, &[root]).expect_err("outside root rejected");
        assert!(error.contains("outside"));

        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn resolve_within_roots_rejects_a_traversal_that_escapes_a_root() {
        let base =
            std::env::temp_dir().join(format!("wayvibes-roots-traversal-{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside-pack");
        write_test_pack(&outside);
        std::fs::create_dir_all(&root).expect("create root directory");
        let traversal = root.join("..").join("outside-pack");

        let error = super::resolve_within_roots(&traversal, &[root])
            .expect_err("a traversal escaping the root is rejected");
        assert!(error.contains("outside"));

        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn set_soundpack_accepts_a_path_inside_a_configured_root_and_persists_it() {
        let (backend, config_path, _rx) = test_backend("soundpack-inside-root");
        let base = config_path.parent().unwrap().to_path_buf();
        let root = base.join("allowed-root");
        let pack = root.join("cream");
        write_test_pack(&pack);
        backend
            .state
            .config
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .soundpack_roots = vec![root];

        let request = format!(r#"{{"cmd":"set_soundpack","path":"{}"}}"#, pack.display());
        let response = handle_request(&backend, &request);

        assert!(response.ok);
        let persisted = load_config(&config_path).expect("load persisted config");
        assert_eq!(
            persisted.selected_soundpack,
            Some(pack.canonicalize().expect("canonicalize pack"))
        );
        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn set_soundpack_rejects_a_path_outside_every_configured_root() {
        let (backend, config_path, _rx) = test_backend("soundpack-outside-root");
        let base = config_path.parent().unwrap().to_path_buf();
        let root = base.join("allowed-root");
        let outside = base.join("elsewhere");
        write_test_pack(&outside);
        std::fs::create_dir_all(&root).expect("create allowed root");
        backend
            .state
            .config
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .soundpack_roots = vec![root];

        let request = format!(
            r#"{{"cmd":"set_soundpack","path":"{}"}}"#,
            outside.display()
        );
        let response = handle_request(&backend, &request);

        assert!(!response.ok);
        assert!(response.error.unwrap().contains("soundpack"));
        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn play_sample_rejects_a_path_outside_every_configured_root() {
        let (backend, config_path, _rx) = test_backend("sample-outside-root");
        let base = config_path.parent().unwrap().to_path_buf();
        let root = base.join("allowed-root");
        let outside = base.join("elsewhere.wav");
        std::fs::create_dir_all(&root).expect("create allowed root");
        std::fs::write(&outside, b"audio").expect("write file outside root");
        backend
            .state
            .config
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .soundpack_roots = vec![root];

        let request = format!(r#"{{"cmd":"play_sample","path":"{}"}}"#, outside.display());
        let response = handle_request(&backend, &request);

        assert!(!response.ok);
        assert!(response.error.unwrap().contains("sample"));
        std::fs::remove_dir_all(base).expect("cleanup");
    }

    #[test]
    fn peer_uid_matches_accepts_a_same_process_socket_pair() {
        let (a, _b) = UnixStream::pair().expect("create socket pair");

        assert!(super::peer_uid_matches(&a));
    }

    #[test]
    fn bind_does_not_unlink_a_socket_when_staleness_cannot_be_determined() {
        use std::os::unix::fs::PermissionsExt;

        let directory =
            std::env::temp_dir().join(format!("wayvibes-bind-guarded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let path = directory.join("guarded.sock");

        let listener = UnixListener::bind(&path).expect("bind first listener");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("remove socket permissions");

        let error = super::bind(&path).expect_err("second bind must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        assert!(
            path.exists(),
            "an undetermined connect failure must not unlink the socket"
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restore socket permissions");
        drop(listener);
        std::fs::remove_dir_all(directory).expect("remove temp directory");
    }
}
