use crate::backend::BackendStatus;
use crate::backend::control::{Request, Response, socket_path};
use crate::backend::stats::Stats;
use crate::config::{AppConfig, clamp_volume};
use cpal::traits::HostTrait;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("could not reach the backend control socket: {0}")]
    Connect(io::Error),
    #[error("could not talk to the backend: {0}")]
    Exchange(io::Error),
    #[error("the backend did not respond in time")]
    Timeout,
    #[error("backend refused the request: {0}")]
    Refused(String),
}

pub fn output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.map(|device| format!("{device}")).collect())
        .unwrap_or_default()
}

pub struct ControlClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl ControlClient {
    pub fn connect() -> Result<Self, ControlError> {
        let path =
            socket_path().map_err(|reason| ControlError::Connect(io::Error::other(reason)))?;

        Self::connect_at(&path)
    }

    pub fn connect_at(path: &Path) -> Result<Self, ControlError> {
        let stream = UnixStream::connect(path).map_err(ControlError::Connect)?;
        stream
            .set_read_timeout(Some(CONTROL_TIMEOUT))
            .map_err(ControlError::Connect)?;
        stream
            .set_write_timeout(Some(CONTROL_TIMEOUT))
            .map_err(ControlError::Connect)?;
        let reader = BufReader::new(stream.try_clone().map_err(ControlError::Exchange)?);

        Ok(Self { stream, reader })
    }

    pub fn status(&mut self) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("status"),
            path: None,
            value: None,
            name: None,
        })
    }

    pub fn set_soundpack(&mut self, path: &Path) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_soundpack"),
            path: Some(path.to_path_buf()),
            value: None,
            name: None,
        })
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_volume"),
            path: None,
            value: Some(clamp_volume(volume)),
            name: None,
        })
    }

    pub fn set_device(&mut self, name: &str) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_device"),
            path: None,
            value: None,
            name: Some(name.to_string()),
        })
    }

    pub fn set_modifier_sounds(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_modifier_sounds"),
            path: None,
            value: Some(f32::from(enabled)),
            name: None,
        })
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_enabled"),
            path: None,
            value: Some(f32::from(enabled)),
            name: None,
        })
    }

    pub fn play_sample(&mut self, path: &Path) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("play_sample"),
            path: Some(path.to_path_buf()),
            value: None,
            name: None,
        })
    }

    pub fn play_ding(&mut self) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("play_ding"),
            path: None,
            value: None,
            name: None,
        })
    }

    pub fn get_stats(&mut self) -> Result<Stats, ControlError> {
        let response = self.command_raw(Request {
            cmd: String::from("get_stats"),
            path: None,
            value: None,
            name: None,
        })?;

        response
            .stats
            .ok_or_else(|| ControlError::Refused(String::from("backend returned no stats")))
    }

    pub fn export_stats(&mut self) -> Result<String, ControlError> {
        let response = self.command_raw(Request {
            cmd: String::from("export_stats"),
            path: None,
            value: None,
            name: None,
        })?;

        response
            .exported
            .ok_or_else(|| ControlError::Refused(String::from("backend returned no export")))
    }

    pub fn reset_stats(&mut self) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("reset_stats"),
            path: None,
            value: None,
            name: None,
        })
    }

    pub fn set_tone_pan(&mut self, pan: f32) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_tone_pan"),
            path: None,
            value: Some(pan.clamp(-1.0, 1.0)),
            name: None,
        })
    }

    pub fn set_tone_distance(&mut self, distance: f32) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_tone_distance"),
            path: None,
            value: Some(distance.clamp(0.0, 1.0)),
            name: None,
        })
    }

    pub fn set_output_device(&mut self, name: Option<&str>) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_output_device"),
            path: None,
            value: None,
            name: name.map(str::to_string),
        })
    }

    pub fn set_return_ding(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_return_ding"),
            path: None,
            value: Some(f32::from(enabled)),
            name: None,
        })
    }

    pub fn set_key_up_sounds(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_key_up_sounds"),
            path: None,
            value: Some(f32::from(enabled)),
            name: None,
        })
    }

    pub fn set_key_up_fallback(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_key_up_fallback"),
            path: None,
            value: Some(f32::from(enabled)),
            name: None,
        })
    }

    pub fn set_pitch_variation(&mut self, variation: f32) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_pitch_variation"),
            path: None,
            value: Some(variation),
            name: None,
        })
    }

    pub fn set_velocity_variation(
        &mut self,
        variation: f32,
    ) -> Result<BackendStatus, ControlError> {
        self.command(Request {
            cmd: String::from("set_velocity_variation"),
            path: None,
            value: Some(variation),
            name: None,
        })
    }

    pub fn apply_config(&mut self, config: &AppConfig) -> Result<BackendStatus, ControlError> {
        let current = self.status()?;
        let requests = pending_requests(config, &current);
        let mut status = current;

        for request in requests {
            status = self.command(request)?;
        }

        Ok(status)
    }

    fn command(&mut self, request: Request) -> Result<BackendStatus, ControlError> {
        map_response(self.command_raw(request)?)
    }

    fn command_raw(&mut self, request: Request) -> Result<Response, ControlError> {
        let payload = serde_json::to_string(&request)
            .map_err(|error| ControlError::Refused(error.to_string()))?;
        self.stream
            .write_all(payload.as_bytes())
            .and_then(|_| self.stream.write_all(b"\n"))
            .map_err(map_exchange_error)?;
        self.stream.flush().map_err(map_exchange_error)?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(map_exchange_error)?;

        serde_json::from_str(&line).map_err(|error| ControlError::Refused(error.to_string()))
    }
}

fn map_exchange_error(error: io::Error) -> ControlError {
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ControlError::Timeout,
        _ => ControlError::Exchange(error),
    }
}

fn pending_requests(config: &AppConfig, current: &BackendStatus) -> Vec<Request> {
    let mut requests = Vec::new();

    if let Some(soundpack) = &config.selected_soundpack {
        requests.push(Request {
            cmd: String::from("set_soundpack"),
            path: Some(soundpack.clone()),
            value: None,
            name: None,
        });
    }

    if (config.volume - current.volume).abs() > f32::EPSILON {
        requests.push(Request {
            cmd: String::from("set_volume"),
            path: None,
            value: Some(clamp_volume(config.volume)),
            name: None,
        });
    }

    if let Some(device) = &config.device_name
        && current.device.as_deref() != Some(device.as_str())
    {
        requests.push(Request {
            cmd: String::from("set_device"),
            path: None,
            value: None,
            name: Some(device.clone()),
        });
    }

    if config.modifier_sounds != current.modifier_sounds {
        requests.push(Request {
            cmd: String::from("set_modifier_sounds"),
            path: None,
            value: Some(f32::from(config.modifier_sounds)),
            name: None,
        });
    }

    if config.key_up_sounds != current.key_up_sounds {
        requests.push(Request {
            cmd: String::from("set_key_up_sounds"),
            path: None,
            value: Some(f32::from(config.key_up_sounds)),
            name: None,
        });
    }

    if config.key_up_fallback != current.key_up_fallback {
        requests.push(Request {
            cmd: String::from("set_key_up_fallback"),
            path: None,
            value: Some(f32::from(config.key_up_fallback)),
            name: None,
        });
    }

    if (config.pitch_variation - current.pitch_variation).abs() > f32::EPSILON {
        requests.push(Request {
            cmd: String::from("set_pitch_variation"),
            path: None,
            value: Some(config.pitch_variation),
            name: None,
        });
    }

    if (config.velocity_variation - current.velocity_variation).abs() > f32::EPSILON {
        requests.push(Request {
            cmd: String::from("set_velocity_variation"),
            path: None,
            value: Some(config.velocity_variation),
            name: None,
        });
    }

    if config.return_ding != current.return_ding {
        requests.push(Request {
            cmd: String::from("set_return_ding"),
            path: None,
            value: Some(f32::from(config.return_ding)),
            name: None,
        });
    }

    if (config.tone_pan - current.tone_pan).abs() > f32::EPSILON {
        requests.push(Request {
            cmd: String::from("set_tone_pan"),
            path: None,
            value: Some(config.tone_pan),
            name: None,
        });
    }

    if (config.tone_distance - current.tone_distance).abs() > f32::EPSILON {
        requests.push(Request {
            cmd: String::from("set_tone_distance"),
            path: None,
            value: Some(config.tone_distance),
            name: None,
        });
    }

    if config.output_device != current.output_device {
        requests.push(Request {
            cmd: String::from("set_output_device"),
            path: None,
            value: None,
            name: config.output_device.clone(),
        });
    }

    requests
}

fn map_response(response: Response) -> Result<BackendStatus, ControlError> {
    if !response.ok {
        return Err(ControlError::Refused(
            response
                .error
                .unwrap_or_else(|| String::from("unknown backend error")),
        ));
    }

    response
        .status
        .ok_or_else(|| ControlError::Refused(String::from("backend returned no status")))
}

#[cfg(test)]
mod tests {
    use super::{ControlClient, map_response, pending_requests};
    use crate::backend::BackendStatus;
    use crate::backend::control::{Request, Response};
    use crate::backend::{BackendInner, BackendState};
    use crate::config::AppConfig;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct LocalSilentAudio {
        volume: AtomicU32,
    }

    impl crate::backend::audio::AudioControl for LocalSilentAudio {
        fn play(&self, _path: &std::path::Path) -> Result<(), crate::backend::audio::AudioError> {
            Ok(())
        }

        fn play_decoded(
            &self,
            _sound: std::sync::Arc<crate::backend::audio::DecodedSound>,
        ) -> Result<(), crate::backend::audio::AudioError> {
            Ok(())
        }

        fn set_master_volume(&self, volume: f32) {
            self.volume.store(volume.to_bits(), Ordering::Relaxed);
        }

        fn volume(&self) -> f32 {
            f32::from_bits(self.volume.load(Ordering::Relaxed))
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

    fn status(soundpack: Option<&str>, volume: f32, device: Option<&str>) -> BackendStatus {
        BackendStatus {
            soundpack: soundpack.map(str::to_string),
            volume,
            device: device.map(str::to_string),
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
        }
    }

    #[test]
    fn requests_round_trip_through_the_server_shape() {
        let cases = [
            Request {
                cmd: String::from("status"),
                path: None,
                value: None,
                name: None,
            },
            Request {
                cmd: String::from("set_volume"),
                path: None,
                value: Some(2.5),
                name: None,
            },
            Request {
                cmd: String::from("set_device"),
                path: None,
                value: None,
                name: Some(String::from("USB Keyboard")),
            },
            Request {
                cmd: String::from("set_soundpack"),
                path: Some(PathBuf::from("/packs/cream")),
                value: None,
                name: None,
            },
        ];

        for request in cases {
            let payload = serde_json::to_string(&request).expect("serialize");
            let parsed: Request = serde_json::from_str(&payload).expect("parse");
            assert_eq!(parsed.cmd, request.cmd);
            assert_eq!(parsed.path, request.path);
            assert_eq!(parsed.value, request.value);
            assert_eq!(parsed.name, request.name);
        }
    }

    #[test]
    fn error_responses_map_to_refused_errors() {
        let response: Response =
            serde_json::from_str(r#"{"ok":false,"error":"no such soundpack"}"#)
                .expect("parse error response");

        let error = map_response(response).expect_err("failed response must error");

        assert!(error.to_string().contains("no such soundpack"));
    }

    #[test]
    fn success_responses_expose_live_status() {
        let response = Response {
            ok: true,
            error: None,
            status: Some(status(Some("Creams"), 3.0, Some("kbd"))),
            stats: None,
            exported: None,
        };

        let mapped = map_response(response).expect("success");

        assert_eq!(mapped.soundpack.as_deref(), Some("Creams"));
        assert_eq!(mapped.volume, 3.0);
    }

    #[test]
    fn status_replies_without_status_are_rejected() {
        let response = Response {
            ok: true,
            error: None,
            status: None,
            stats: None,
            exported: None,
        };

        assert!(map_response(response).is_err());
    }

    #[test]
    fn client_talks_to_the_backend_over_a_real_socket() {
        let directory =
            std::env::temp_dir().join(format!("wayvibes-client-socket-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        let socket = directory.join("udu.sock");
        let config_path = directory.join("config.json");

        let (tx, _rx) = mpsc::channel();
        let backend = Arc::new(BackendInner {
            audio: Arc::new(LocalSilentAudio {
                volume: AtomicU32::new(1.0f32.to_bits()),
            }),
            state: BackendState {
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
        });

        let server_backend = Arc::clone(&backend);
        let server_socket = socket.clone();
        let server = std::thread::spawn(move || {
            let _ = crate::backend::control::serve(&server_socket, &server_backend);
        });
        std::thread::sleep(Duration::from_millis(150));

        let mut client = ControlClient::connect_at(&socket).expect("connect");
        let status = client.status().expect("status");
        assert_eq!(status.volume, 1.0);

        let status = client.set_volume(4.0).expect("set volume");
        assert_eq!(status.volume, 4.0);

        let persisted = crate::config::load_config(&config_path).expect("persisted config");
        assert_eq!(persisted.volume, 4.0);

        let status = client.status().expect("status again");
        assert_eq!(status.volume, 4.0);

        drop(client);
        drop(server);
        drop(backend);
        std::fs::remove_dir_all(directory).expect("remove temp directory");
    }

    #[test]
    fn pending_requests_only_emit_volume_and_device_when_changed() {
        let config = AppConfig {
            soundpack_roots: Vec::new(),
            selected_soundpack: Some(PathBuf::from("/packs/cream")),
            volume: 2.0,
            volume_scale_version: crate::config::CURRENT_VOLUME_SCALE_VERSION,
            device_name: Some(String::from("kbd")),
            modifier_sounds: true,
            key_up_sounds: true,
            key_up_fallback: true,
            pitch_variation: crate::config::DEFAULT_PITCH_VARIATION,
            velocity_variation: crate::config::DEFAULT_VELOCITY_VARIATION,
            return_ding: false,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 1.0,
        };
        let current = status(Some("Creams"), 2.0, Some("kbd"));

        let requests = pending_requests(&config, &current);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].cmd, "set_soundpack");
    }

    #[test]
    fn pending_requests_emit_each_difference_once() {
        let config = AppConfig {
            soundpack_roots: Vec::new(),
            selected_soundpack: Some(PathBuf::from("/packs/other")),
            volume: 5.0,
            volume_scale_version: crate::config::CURRENT_VOLUME_SCALE_VERSION,
            device_name: Some(String::from("other")),
            modifier_sounds: true,
            key_up_sounds: true,
            key_up_fallback: true,
            pitch_variation: crate::config::DEFAULT_PITCH_VARIATION,
            velocity_variation: crate::config::DEFAULT_VELOCITY_VARIATION,
            return_ding: false,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 1.0,
        };
        let current = status(Some("Creams"), 2.0, Some("kbd"));

        let requests = pending_requests(&config, &current);

        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].cmd, "set_soundpack");
        assert_eq!(requests[1].cmd, "set_volume");
        assert_eq!(requests[2].cmd, "set_device");
    }
}
