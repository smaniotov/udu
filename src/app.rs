use crate::config::{AppConfig, MAX_VARIATION, clamp_volume};
use crate::control::{ControlClient, ControlError, output_devices};
use crate::device::{KeyboardDevice, discover_keyboards};
use crate::service::{LegacyUnitMigration, LegacyUnitOutcome, UduService};
use crate::soundpack::{Soundpack, SoundpackError, discover_soundpacks, validate_soundpack};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::backend::BackendStatus;
use anyhow::Result;
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Launcher,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Audio,
    Devices,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceModal {
    InstallConsent,
    ConfirmUninstall,
}

pub const SETTINGS_TAB_COUNT: usize = 4;
const SERVICE_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub(crate) struct SettingRow {
    pub(crate) label: &'static str,
    kind: SettingKind,
}

enum SettingKind {
    Toggle {
        read: fn(&AppConfig) -> bool,
        write: fn(&mut AppConfig, bool),
    },
    Ranged {
        read: fn(&AppConfig) -> f32,
        write: fn(&mut AppConfig, f32),
        step: f32,
        range: (f32, f32),
        format: fn(f32) -> String,
    },
    Device,
}

pub(crate) enum SettingTone {
    On,
    Off,
    Plain,
    Accent,
}

impl SettingRow {
    pub(crate) fn value_text(&self, config: &AppConfig) -> String {
        match &self.kind {
            SettingKind::Toggle { read, .. } => on_off(read(config)).to_string(),
            SettingKind::Ranged { read, format, .. } => format(read(config)),
            SettingKind::Device => config
                .output_device
                .as_deref()
                .unwrap_or("default")
                .to_string(),
        }
    }

    pub(crate) fn tone(&self, config: &AppConfig) -> SettingTone {
        match &self.kind {
            SettingKind::Toggle { read, .. } if read(config) => SettingTone::On,
            SettingKind::Toggle { .. } => SettingTone::Off,
            SettingKind::Ranged { .. } => SettingTone::Plain,
            SettingKind::Device => SettingTone::Accent,
        }
    }
}

pub(crate) const GENERAL_SETTINGS: [SettingRow; 4] = [
    SettingRow {
        label: "Key-up sounds",
        kind: SettingKind::Toggle {
            read: |config| config.key_up_sounds,
            write: |config, value| config.key_up_sounds = value,
        },
    },
    SettingRow {
        label: "Key-up fallback",
        kind: SettingKind::Toggle {
            read: |config| config.key_up_fallback,
            write: |config, value| config.key_up_fallback = value,
        },
    },
    SettingRow {
        label: "Modifier sounds",
        kind: SettingKind::Toggle {
            read: |config| config.modifier_sounds,
            write: |config, value| config.modifier_sounds = value,
        },
    },
    SettingRow {
        label: "Return ding",
        kind: SettingKind::Toggle {
            read: |config| config.return_ding,
            write: |config, value| config.return_ding = value,
        },
    },
];

pub(crate) const AUDIO_SETTINGS: [SettingRow; 5] = [
    SettingRow {
        label: "Pitch variation",
        kind: SettingKind::Ranged {
            read: |config| config.pitch_variation,
            write: |config, value| config.pitch_variation = value,
            step: 0.01,
            range: (0.0, MAX_VARIATION),
            format: |value| format!("{:.0}%", value * 100.0),
        },
    },
    SettingRow {
        label: "Velocity variation",
        kind: SettingKind::Ranged {
            read: |config| config.velocity_variation,
            write: |config, value| config.velocity_variation = value,
            step: 0.01,
            range: (0.0, MAX_VARIATION),
            format: |value| format!("{:.0}%", value * 100.0),
        },
    },
    SettingRow {
        label: "Tone pan",
        kind: SettingKind::Ranged {
            read: |config| config.tone_pan,
            write: |config, value| config.tone_pan = value,
            step: 0.02,
            range: (-1.0, 1.0),
            format: |value| format!("{value:.1}"),
        },
    },
    SettingRow {
        label: "Tone distance",
        kind: SettingKind::Ranged {
            read: |config| config.tone_distance,
            write: |config, value| config.tone_distance = value,
            step: 0.02,
            range: (0.0, 1.0),
            format: |value| format!("{value:.1}"),
        },
    },
    SettingRow {
        label: "Output device",
        kind: SettingKind::Device,
    },
];

pub const GENERAL_SETTING_COUNT: usize = GENERAL_SETTINGS.len();
pub const AUDIO_VALUE_COUNT: usize = AUDIO_SETTINGS.len();

fn current_setting_row(tab: SettingsTab, index: usize) -> Option<&'static SettingRow> {
    match tab {
        SettingsTab::General => GENERAL_SETTINGS.get(index),
        SettingsTab::Audio => AUDIO_SETTINGS.get(index),
        SettingsTab::Devices | SettingsTab::About => None,
    }
}

pub trait BackendControl {
    fn status(&mut self) -> Result<BackendStatus, ControlError>;
    fn set_soundpack(&mut self, path: &Path) -> Result<BackendStatus, ControlError>;
    fn set_volume(&mut self, volume: f32) -> Result<BackendStatus, ControlError>;
    fn set_device(&mut self, name: &str) -> Result<BackendStatus, ControlError>;
    fn set_enabled(&mut self, enabled: bool) -> Result<BackendStatus, ControlError>;
    fn play_sample(&mut self, path: &Path) -> Result<BackendStatus, ControlError>;
    fn play_ding(&mut self) -> Result<BackendStatus, ControlError>;
    fn get_stats(&mut self) -> Result<crate::backend::stats::Stats, ControlError>;
    fn set_tone_pan(&mut self, pan: f32) -> Result<BackendStatus, ControlError>;
    fn set_tone_distance(&mut self, distance: f32) -> Result<BackendStatus, ControlError>;
    fn apply_config(&mut self, config: &AppConfig) -> Result<BackendStatus, ControlError>;
}

impl BackendControl for ControlClient {
    fn status(&mut self) -> Result<BackendStatus, ControlError> {
        ControlClient::status(self)
    }

    fn set_soundpack(&mut self, path: &Path) -> Result<BackendStatus, ControlError> {
        ControlClient::set_soundpack(self, path)
    }

    fn set_volume(&mut self, volume: f32) -> Result<BackendStatus, ControlError> {
        ControlClient::set_volume(self, volume)
    }

    fn set_device(&mut self, name: &str) -> Result<BackendStatus, ControlError> {
        ControlClient::set_device(self, name)
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
        ControlClient::set_enabled(self, enabled)
    }

    fn play_sample(&mut self, path: &Path) -> Result<BackendStatus, ControlError> {
        ControlClient::play_sample(self, path)
    }

    fn play_ding(&mut self) -> Result<BackendStatus, ControlError> {
        ControlClient::play_ding(self)
    }

    fn get_stats(&mut self) -> Result<crate::backend::stats::Stats, ControlError> {
        ControlClient::get_stats(self)
    }

    fn set_tone_pan(&mut self, pan: f32) -> Result<BackendStatus, ControlError> {
        ControlClient::set_tone_pan(self, pan)
    }

    fn set_tone_distance(&mut self, distance: f32) -> Result<BackendStatus, ControlError> {
        ControlClient::set_tone_distance(self, distance)
    }

    fn apply_config(&mut self, config: &AppConfig) -> Result<BackendStatus, ControlError> {
        ControlClient::apply_config(self, config)
    }
}

pub struct App {
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub packs: Vec<Soundpack>,
    pub list_state: ListState,
    pub devices: Vec<KeyboardDevice>,
    pub device_list_state: ListState,
    pub screen: Screen,
    pub service_modal: Option<ServiceModal>,
    pub settings_tab: SettingsTab,
    pub settings_index: usize,
    pub search_query: String,
    pub status: String,
    pub should_quit: bool,
    service: UduService,
    backend: Option<Box<dyn BackendControl>>,
    last_service_check: Option<Instant>,
    pub sound_enabled: bool,
    pub output_devices: Vec<String>,
}

impl App {
    pub fn new(config_path: PathBuf, config: AppConfig) -> Result<Self> {
        let mut app = Self {
            config,
            config_path,
            packs: Vec::new(),
            list_state: ListState::default(),
            devices: Vec::new(),
            device_list_state: ListState::default(),
            screen: Screen::Launcher,
            service_modal: None,
            settings_tab: SettingsTab::General,
            settings_index: 0,
            search_query: String::new(),
            status: String::from("Loading soundpacks and devices..."),
            should_quit: false,
            service: UduService,
            backend: None,
            last_service_check: None,
            sound_enabled: true,
            output_devices: output_devices(),
        };

        app.refresh_soundpacks()?;
        app.refresh_devices();

        Ok(app)
    }

    pub fn selected_pack(&self) -> Option<&Soundpack> {
        let selected = self.list_state.selected();
        let pack_index =
            selected.and_then(|index| self.filtered_pack_indices().get(index).copied());
        pack_index.and_then(|index| self.packs.get(index))
    }

    pub fn visible_packs(&self) -> Vec<&Soundpack> {
        self.filtered_pack_indices()
            .into_iter()
            .filter_map(|index| self.packs.get(index))
            .collect()
    }

    pub fn selected_pack_name(&self) -> Option<String> {
        self.selected_pack().map(|pack| pack.name.clone())
    }

    pub fn selected_device(&self) -> Option<&KeyboardDevice> {
        self.device_list_state
            .selected()
            .and_then(|index| self.devices.get(index))
    }

    pub fn select_next(&mut self) {
        self.move_selection(true);
    }

    pub fn select_previous(&mut self) {
        self.move_selection(false);
    }

    fn filtered_pack_indices(&self) -> Vec<usize> {
        if self.search_query.is_empty() {
            return (0..self.packs.len()).collect();
        }

        let query = self.search_query.to_lowercase();
        self.packs
            .iter()
            .enumerate()
            .filter(|(_, pack)| pack.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn type_search(&mut self, c: char) {
        self.search_query.push(c);
        let matches = self.filtered_pack_indices();
        if matches.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn backspace_search(&mut self) {
        self.search_query.pop();
        let matches = self.filtered_pack_indices();
        self.list_state
            .select(matches.first().map(|_| 0).or(Some(0)));
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        if !self.packs.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn move_selection(&mut self, forward: bool) {
        match self.screen {
            Screen::Launcher => self.move_pack_selection(forward),
            Screen::Settings => match self.settings_tab {
                SettingsTab::General => self.move_index(forward, GENERAL_SETTING_COUNT),
                SettingsTab::Audio => self.move_index(forward, AUDIO_VALUE_COUNT),
                SettingsTab::Devices => self.move_device_selection(forward),
                SettingsTab::About => {}
            },
        }
    }

    fn move_pack_selection(&mut self, forward: bool) {
        let length = self.filtered_pack_indices().len();
        if length == 0 {
            self.list_state.select(None);
            return;
        }

        let current = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if forward {
            (current + 1) % length
        } else {
            current.checked_sub(1).unwrap_or(length - 1)
        }));
    }

    fn move_device_selection(&mut self, forward: bool) {
        let length = self.devices.len();
        if length == 0 {
            self.device_list_state.select(None);
            return;
        }

        let current = self.device_list_state.selected().unwrap_or(0);
        self.device_list_state.select(Some(if forward {
            (current + 1) % length
        } else {
            current.checked_sub(1).unwrap_or(length - 1)
        }));
    }

    fn move_index(&mut self, forward: bool, count: usize) {
        self.settings_index = if forward {
            (self.settings_index + 1) % count
        } else {
            self.settings_index.checked_sub(1).unwrap_or(count - 1)
        };
    }

    pub fn open_settings(&mut self) {
        self.screen = Screen::Settings;
        self.settings_index = 0;
    }

    pub fn close_settings(&mut self) {
        self.screen = Screen::Launcher;
    }

    pub fn cycle_settings_tab(&mut self, forward: bool) {
        let tabs = [
            SettingsTab::General,
            SettingsTab::Audio,
            SettingsTab::Devices,
            SettingsTab::About,
        ];
        let current = tabs
            .iter()
            .position(|tab| *tab == self.settings_tab)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % SETTINGS_TAB_COUNT
        } else {
            (current + SETTINGS_TAB_COUNT - 1) % SETTINGS_TAB_COUNT
        };
        self.settings_tab = tabs[next];
        self.settings_index = 0;
    }

    pub fn activate_selected(&mut self) -> Result<()> {
        match self.screen {
            Screen::Launcher => self.activate_selected_soundpack(),
            Screen::Settings => match self.settings_tab {
                SettingsTab::General | SettingsTab::Audio => self.activate_setting_row(),
                SettingsTab::Devices => self.activate_selected_device(),
                SettingsTab::About => {
                    self.close_settings();
                    Ok(())
                }
            },
        }
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.refresh_soundpacks()?;
        let has_devices = self.refresh_devices();

        if has_devices {
            self.status = format!(
                "Found {} soundpack(s) and {} keyboard device(s).",
                self.packs.len(),
                self.devices.len()
            );
        }

        Ok(())
    }

    pub fn apply_preset(&mut self, preset: f32) -> Result<()> {
        let outcome = self.backend_mut().map(|backend| backend.set_volume(preset));
        let status = match outcome {
            Some(Ok(status)) => status,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };

        self.config.volume = status.volume;
        self.sound_enabled = status.enabled;
        self.status = format!("Volume: {:.1}", self.config.volume);
        self.play_cue();

        Ok(())
    }

    pub fn preview_selected(&mut self) -> Result<()> {
        let Some(pack) = self.selected_pack().cloned() else {
            self.status = String::from("Select a soundpack to preview.");
            return Ok(());
        };
        let sample = crate::soundpack::parse_pack(&pack.path)
            .ok()
            .and_then(|parsed| parsed.defines.values().next().cloned());
        let outcome = match sample {
            Some(sample) => self
                .backend_mut()
                .map(|backend| backend.play_sample(&sample)),
            None => {
                self.status = format!("{} has no playable sounds.", pack.name);
                return Ok(());
            }
        };
        let status = match outcome {
            Some(Ok(status)) => status,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };

        self.sound_enabled = status.enabled;
        self.status = format!("Preview: {}", pack.name);

        Ok(())
    }

    pub fn show_stats(&mut self) -> Result<()> {
        let outcome = self.backend_mut().map(|backend| backend.get_stats());
        let stats = match outcome {
            Some(Ok(stats)) => stats,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };

        let per_switch = stats.per_switch.len();
        self.status = format!(
            "Stats: {} keys · {} dings · {} switches",
            stats.keystrokes, stats.dings, per_switch
        );

        Ok(())
    }

    fn activate_setting_row(&mut self) -> Result<()> {
        let Some(row) = current_setting_row(self.settings_tab, self.settings_index) else {
            return Ok(());
        };

        match &row.kind {
            SettingKind::Toggle { read, write } => {
                let next = !read(&self.config);
                write(&mut self.config, next);
            }
            SettingKind::Ranged {
                read,
                write,
                step,
                range,
                ..
            } => {
                let next = (read(&self.config) + step).clamp(range.0, range.1);
                write(&mut self.config, next);
            }
            SettingKind::Device => self.select_output_device(1.0),
        }

        self.apply_audio_config()
    }

    pub fn adjust_audio_setting(&mut self, direction: f32) -> Result<()> {
        let Some(row) = current_setting_row(SettingsTab::Audio, self.settings_index) else {
            self.status = String::from("This is not an adjustable value.");
            return Ok(());
        };

        match &row.kind {
            SettingKind::Ranged {
                read,
                write,
                step,
                range,
                ..
            } => {
                let next = (read(&self.config) + step * direction).clamp(range.0, range.1);
                write(&mut self.config, next);
            }
            SettingKind::Device => self.select_output_device(direction),
            SettingKind::Toggle { .. } => {
                self.status = String::from("This is not an adjustable value.");
                return Ok(());
            }
        }

        self.apply_audio_config()
    }

    fn select_output_device(&mut self, delta: f32) {
        let choices = std::iter::once(None)
            .chain(self.output_devices.iter().cloned().map(Some))
            .collect::<Vec<_>>();
        let current = self
            .config
            .output_device
            .as_ref()
            .and_then(|name| {
                choices
                    .iter()
                    .position(|choice| choice.as_ref() == Some(name))
            })
            .unwrap_or(0);
        let next = if delta.is_sign_positive() {
            (current + 1) % choices.len()
        } else {
            current.checked_sub(1).unwrap_or(choices.len() - 1)
        };
        self.config.output_device = choices[next].clone();
    }

    fn sync_audio_status(&mut self, status: &BackendStatus) {
        self.config.key_up_sounds = status.key_up_sounds;
        self.config.key_up_fallback = status.key_up_fallback;
        self.config.modifier_sounds = status.modifier_sounds;
        self.config.return_ding = status.return_ding;
        self.config.pitch_variation = status.pitch_variation;
        self.config.velocity_variation = status.velocity_variation;
        self.config.tone_pan = status.tone_pan;
        self.config.tone_distance = status.tone_distance;
        self.config.output_device = status.output_device.clone();
    }

    fn apply_audio_config(&mut self) -> Result<()> {
        let config = self.config.clone();
        let outcome = self
            .backend_mut()
            .map(|backend| backend.apply_config(&config));
        let status = match outcome {
            Some(Ok(status)) => status,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };

        self.sync_audio_status(&status);
        self.status = audio_setting_status(&self.config, self.settings_tab, self.settings_index);

        Ok(())
    }

    pub fn toggle_sound(&mut self) -> Result<()> {
        let next = !self.sound_enabled;
        let outcome = self.backend_mut().map(|backend| backend.set_enabled(next));
        let status = match outcome {
            Some(Ok(status)) => status,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };

        self.sound_enabled = status.enabled;
        self.status = if self.sound_enabled {
            String::from("Sounds: on")
        } else {
            String::from("Sounds: muted")
        };

        Ok(())
    }

    pub fn adjust_volume(&mut self, delta: f32) -> Result<()> {
        let volume = clamp_volume(self.config.volume + delta);
        let Some(backend) = self.backend_mut() else {
            self.status = String::from("Backend not connected: run the service and reopen.");
            return Ok(());
        };

        let status = match backend.set_volume(volume) {
            Ok(status) => status,
            Err(error) => return self.report_backend_error(error),
        };
        self.config.volume = status.volume;
        self.status = format!("Volume: {:.1}", self.config.volume);

        Ok(())
    }

    pub fn poll_process(&mut self) -> Result<()> {
        if !is_stale(
            self.last_service_check,
            Instant::now(),
            SERVICE_POLL_INTERVAL,
        ) {
            return Ok(());
        }

        self.last_service_check = Some(Instant::now());
        let Some(backend) = self.backend_mut() else {
            return Ok(());
        };

        match backend.status() {
            Ok(status) => {
                self.sound_enabled = status.enabled;
                self.sync_audio_status(&status);
                self.status = live_status_line(&status);
            }
            Err(error) => {
                self.status = format!("Backend unreachable ({error}). Restart the service.");
            }
        }

        Ok(())
    }

    pub fn start_backend(&mut self) -> Result<()> {
        if self.service.is_installed() {
            return self.install_and_connect_backend();
        }

        self.service_modal = Some(ServiceModal::InstallConsent);
        self.status = String::from(
            "The udu service is not installed yet. Review the consent prompt to enable sound.",
        );

        Ok(())
    }

    pub fn service_installed(&self) -> bool {
        self.service.is_installed()
    }

    pub fn grant_service_consent(&mut self) -> Result<()> {
        self.service_modal = None;

        if let Err(error) = self.install_and_connect_backend() {
            self.status = format!("Could not install the udu service: {error}");
        }

        Ok(())
    }

    pub fn decline_service_consent(&mut self) {
        self.service_modal = None;
        self.status = String::from(
            "udu service not installed; sounds are off. Restart udu to be asked again.",
        );
    }

    pub fn request_uninstall_confirmation(&mut self) {
        if !self.service.is_installed() {
            return;
        }

        self.service_modal = Some(ServiceModal::ConfirmUninstall);
    }

    pub fn cancel_uninstall_service(&mut self) {
        self.service_modal = None;
    }

    pub fn confirm_uninstall_service(&mut self) -> Result<()> {
        self.service_modal = None;

        match self.service.stop_and_uninstall() {
            Ok(()) => {
                self.backend = None;
                self.status =
                    String::from("udu service removed. Sounds are off until you reinstall it.");
            }
            Err(error) => {
                self.status = format!("Could not remove the udu service: {error}");
            }
        }

        Ok(())
    }

    fn install_and_connect_backend(&mut self) -> Result<()> {
        let migrations = self
            .service
            .start_service(&self.config_path, &self.config)?;

        let mut client = match ControlClient::connect() {
            Ok(client) => client,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(300));
                ControlClient::connect()?
            }
        };

        let status = client.apply_config(&self.config)?;
        self.config.volume = status.volume;
        self.sound_enabled = status.enabled;
        self.sync_audio_status(&status);
        self.status = install_status_line(&status, &migrations);
        self.backend = Some(Box::new(client));

        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    fn backend_mut(&mut self) -> Option<&mut (dyn BackendControl + 'static)> {
        self.backend.as_mut().map(|backend| backend.as_mut())
    }

    fn activate_selected_soundpack(&mut self) -> Result<()> {
        let Some(pack) = self.selected_pack().cloned() else {
            self.status = String::from("No valid soundpack is available.");
            return Ok(());
        };

        validate_soundpack(&pack.path)?;
        let outcome = self
            .backend_mut()
            .map(|backend| backend.set_soundpack(&pack.path));
        let status = match outcome {
            Some(Ok(status)) => status,
            Some(Err(error)) => return self.report_backend_error(error),
            None => {
                self.status = String::from("Backend not connected: run the service and reopen.");
                return Ok(());
            }
        };
        self.config.selected_soundpack = Some(pack.path.clone());
        self.config.volume = status.volume;
        self.sound_enabled = status.enabled;
        self.status = format!("Soundpack: {} (active)", pack.name);
        self.play_cue();

        Ok(())
    }

    fn activate_selected_device(&mut self) -> Result<()> {
        let Some(device) = self.selected_device().cloned() else {
            self.status = String::from("No keyboard device is available.");
            return Ok(());
        };

        let Some(backend) = self.backend_mut() else {
            self.status = String::from("Backend not connected: run the service and reopen.");
            return Ok(());
        };

        let status = match backend.set_device(&device.name) {
            Ok(status) => status,
            Err(error) => return self.report_backend_error(error),
        };
        self.config.device_name = Some(device.name.clone());
        self.config.volume = status.volume;
        self.sound_enabled = status.enabled;
        self.status = format!("Device: {} (active)", device.name);

        Ok(())
    }

    fn play_cue(&mut self) {
        if let Some(backend) = self.backend_mut() {
            let _ = backend.play_ding();
        }
    }

    fn report_backend_error(&mut self, error: crate::control::ControlError) -> Result<()> {
        self.status = format!("Backend unreachable ({error}). Run the service and reopen.");

        Ok(())
    }

    fn refresh_soundpacks(&mut self) -> Result<()> {
        let mut packs = discover_soundpacks(&self.config.soundpack_roots)?;

        if let Some(selected_path) = &self.config.selected_soundpack {
            add_explicit_pack(&mut packs, selected_path)?;
        }

        packs.sort_by(|left, right| left.name.cmp(&right.name));
        let index = selected_pack_index(&packs, self.config.selected_soundpack.as_deref());
        self.packs = packs;
        self.list_state = ListState::default();
        if !self.packs.is_empty() {
            self.list_state.select(Some(index));
        }

        Ok(())
    }

    fn refresh_devices(&mut self) -> bool {
        match discover_keyboards() {
            Ok(devices) => {
                let index = selected_device_index(&devices, self.config.device_name.as_deref());
                self.devices = devices;
                self.device_list_state = ListState::default();
                if !self.devices.is_empty() {
                    self.device_list_state.select(Some(index));
                }

                true
            }
            Err(error) => {
                self.devices.clear();
                self.device_list_state = ListState::default();
                self.status = error.to_string();

                false
            }
        }
    }
}

fn audio_setting_status(config: &AppConfig, tab: SettingsTab, index: usize) -> String {
    let Some(row) = current_setting_row(tab, index) else {
        return String::new();
    };

    format!("{}: {}", row.label, row.value_text(config))
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

fn live_status_line(status: &BackendStatus) -> String {
    if !status.enabled {
        return String::from("Sounds muted (x to toggle)");
    }

    if status.stream_failed {
        return String::from("Audio stream failed; restart the backend service.");
    }

    if !status.device_connected {
        return String::from("Backend running, but no keyboard device is connected.");
    }

    format!(
        "{} · volume {:.1}",
        status.soundpack.as_deref().unwrap_or("no soundpack"),
        status.volume
    )
}

fn install_status_line(status: &BackendStatus, migrations: &[LegacyUnitMigration]) -> String {
    let base = live_status_line(status);
    let skipped_foreign = migrations
        .iter()
        .any(|migration| matches!(migration.outcome, LegacyUnitOutcome::SkippedNotOwnedByUdu));

    if skipped_foreign {
        return format!("{base} · a legacy unit was left untouched (not owned by udu)");
    }

    base
}

fn is_stale(last_check: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last_check.is_none_or(|last| now.duration_since(last) >= interval)
}

fn add_explicit_pack(packs: &mut Vec<Soundpack>, path: &Path) -> Result<(), SoundpackError> {
    if packs.iter().any(|pack| pack.path == path) {
        return Ok(());
    }

    packs.push(validate_soundpack(path)?);

    Ok(())
}

fn selected_pack_index(packs: &[Soundpack], selected_path: Option<&Path>) -> usize {
    selected_path
        .and_then(|path| packs.iter().position(|pack| pack.path == path))
        .unwrap_or(0)
}

fn selected_device_index(devices: &[KeyboardDevice], selected_name: Option<&str>) -> usize {
    selected_name
        .and_then(|name| devices.iter().position(|device| device.name == name))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        App, BackendControl, Screen, ServiceModal, SettingsTab, is_stale, live_status_line,
    };
    use crate::backend::BackendStatus;
    use crate::config::{AppConfig, clamp_volume};
    use crate::control::ControlError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct FakeBackend {
        volume: f32,
        soundpack: Option<String>,
        device: Option<String>,
        connected: bool,
        stream_failed: bool,
        enabled: bool,
        tone_pan: f32,
        tone_distance: f32,
        commands: Vec<String>,
    }

    impl FakeBackend {
        fn current(&self) -> BackendStatus {
            BackendStatus {
                soundpack: self.soundpack.clone(),
                volume: self.volume,
                device: self.device.clone(),
                device_connected: self.connected,
                stream_failed: self.stream_failed,
                output_device: None,
                tone_pan: self.tone_pan,
                tone_distance: self.tone_distance,
                enabled: self.enabled,
                modifier_sounds: true,
                key_up_sounds: true,
                key_up_fallback: true,
                pitch_variation: crate::config::DEFAULT_PITCH_VARIATION,
                velocity_variation: crate::config::DEFAULT_VELOCITY_VARIATION,
                return_ding: false,
            }
        }
    }

    impl BackendControl for FakeBackend {
        fn status(&mut self) -> Result<BackendStatus, ControlError> {
            Ok(self.current())
        }

        fn set_soundpack(&mut self, path: &Path) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_soundpack"));
            self.soundpack = Some(path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into(),
            ));
            Ok(self.current())
        }

        fn set_volume(&mut self, volume: f32) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_volume"));
            self.volume = clamp_volume(volume);
            Ok(self.current())
        }

        fn set_device(&mut self, name: &str) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_device"));
            self.device = Some(name.to_string());
            Ok(self.current())
        }

        fn set_enabled(&mut self, enabled: bool) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_enabled"));
            self.enabled = enabled;
            Ok(self.current())
        }

        fn play_sample(&mut self, _path: &Path) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("play_sample"));
            Ok(self.current())
        }

        fn play_ding(&mut self) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("play_ding"));
            Ok(self.current())
        }

        fn get_stats(&mut self) -> Result<crate::backend::stats::Stats, ControlError> {
            Ok(crate::backend::stats::Stats {
                keystrokes: 0,
                dings: 0,
                since: String::from("t"),
                per_switch: std::collections::BTreeMap::new(),
            })
        }

        fn set_tone_pan(&mut self, pan: f32) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_tone_pan"));
            let _ = pan;
            Ok(self.current())
        }

        fn set_tone_distance(&mut self, distance: f32) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("set_tone_distance"));
            let _ = distance;
            Ok(self.current())
        }

        fn apply_config(&mut self, _config: &AppConfig) -> Result<BackendStatus, ControlError> {
            self.commands.push(String::from("apply_config"));
            Ok(self.current())
        }
    }

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-app-{name}-{}", std::process::id()))
    }

    fn write_pack(directory: &Path, name: &str) {
        fs::create_dir_all(directory).expect("create test pack");
        fs::write(
            directory.join("config.json"),
            format!(r#"{{"name":"{name}","defines":{{"30":"key.wav"}}}}"#),
        )
        .expect("write test config");
        fs::write(directory.join("key.wav"), b"audio").expect("write test audio");
    }

    fn test_app(name: &str) -> (App, PathBuf) {
        let root = test_directory(name);
        let config_path = root.join("config.json");
        fs::create_dir_all(&root).expect("create test directory");
        let app = App::new(config_path.clone(), AppConfig::default()).expect("create app");
        (app, root)
    }

    #[test]
    fn discovers_an_explicit_pack_without_a_search_root() {
        let root = test_directory("explicit-pack");
        let pack_path = root.join("quiet");
        let config_path = root.join("config.json");
        write_pack(&pack_path, "Quiet Keys");

        let config = AppConfig {
            selected_soundpack: Some(pack_path.clone()),
            ..AppConfig::default()
        };
        let app = App::new(config_path, config).expect("create app");

        assert_eq!(
            app.selected_pack().map(|pack| pack.path.clone()),
            Some(pack_path)
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn volume_adjust_is_applied_live_and_clamped_by_the_backend() {
        let (mut app, root) = test_app("volume");
        let backend = FakeBackend {
            volume: 10.0,
            ..FakeBackend::default()
        };
        app.backend = Some(Box::new(backend));

        app.adjust_volume(20.0).expect("increase volume");

        assert_eq!(app.config.volume, 30.0);
        assert_eq!(app.status, "Volume: 30.0");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn adjusting_volume_without_a_backend_reports_unconnected() {
        let (mut app, root) = test_app("no-backend");

        app.adjust_volume(1.0).expect("no-op");

        assert!(app.status.contains("Backend not connected"));
        assert_eq!(app.config.volume, AppConfig::default().volume);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn activating_a_pack_sends_a_live_request_and_updates_status() {
        let root = test_directory("activate-pack");
        write_pack(&root.join("cream"), "Creams");
        let config_path = root.join("config.json");
        let config = AppConfig {
            soundpack_roots: vec![root.clone()],
            ..AppConfig::default()
        };
        let mut app = App::new(config_path, config).expect("create app");
        let backend = FakeBackend::default();
        app.backend = Some(Box::new(backend));

        app.activate_selected().expect("activate pack");

        let expected = root.join("cream");
        assert_eq!(
            app.config.selected_soundpack.as_deref(),
            Some(expected.as_path())
        );
        assert!(app.status.contains("Soundpack: Creams"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn activating_a_device_sends_a_live_request() {
        let root = test_directory("activate-device");
        let config_path = root.join("config.json");
        fs::create_dir_all(&root).expect("create test directory");
        let mut app = App::new(config_path, AppConfig::default()).expect("create app");
        app.devices = vec![crate::device::KeyboardDevice {
            name: String::from("USB Keyboard"),
            path: PathBuf::from("/dev/input/event0"),
        }];
        app.device_list_state.select(Some(0));
        app.screen = Screen::Settings;
        app.settings_tab = SettingsTab::Devices;
        let backend = FakeBackend::default();
        app.backend = Some(Box::new(backend));

        app.activate_selected().expect("activate device");

        assert_eq!(app.config.device_name.as_deref(), Some("USB Keyboard"));
        assert!(app.status.contains("Device: USB Keyboard"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn mute_toggle_updates_state_and_status() {
        let (mut app, root) = test_app("mute");
        let backend = FakeBackend {
            enabled: true,
            ..FakeBackend::default()
        };
        app.backend = Some(Box::new(backend));

        app.toggle_sound().expect("mute");

        assert!(!app.sound_enabled);
        assert_eq!(app.status, "Sounds: muted");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn presets_apply_their_volume_level_to_the_backend() {
        let (mut app, root) = test_app("presets");
        let backend = FakeBackend {
            volume: 10.0,
            ..FakeBackend::default()
        };
        app.backend = Some(Box::new(backend));

        app.apply_preset(crate::config::VOLUME_LOUD).expect("loud");

        assert_eq!(app.config.volume, crate::config::VOLUME_LOUD);
        assert_eq!(app.status, "Volume: 90.0");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn preview_plays_the_first_pack_sample() {
        let root = test_directory("preview");
        write_pack(&root.join("cream"), "Creams");
        let config_path = root.join("config.json");
        let config = AppConfig {
            soundpack_roots: vec![root.clone()],
            ..AppConfig::default()
        };
        let mut app = App::new(config_path, config).expect("create app");
        let backend = FakeBackend::default();
        app.backend = Some(Box::new(backend));

        app.preview_selected().expect("preview");

        assert_eq!(app.status, "Preview: Creams");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn opens_and_closes_settings() {
        let (mut app, root) = test_app("settings");

        assert_eq!(app.screen, Screen::Launcher);

        app.open_settings();
        assert_eq!(app.screen, Screen::Settings);

        app.close_settings();
        assert_eq!(app.screen, Screen::Launcher);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn declining_the_install_consent_closes_the_modal_and_explains_the_status() {
        let (mut app, root) = test_app("decline-consent");
        app.service_modal = Some(ServiceModal::InstallConsent);

        app.decline_service_consent();

        assert_eq!(app.service_modal, None);
        assert!(app.status.contains("not installed"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn canceling_the_uninstall_confirmation_closes_the_modal_without_acting() {
        let (mut app, root) = test_app("cancel-uninstall");
        app.service_modal = Some(ServiceModal::ConfirmUninstall);
        app.status = String::from("unchanged");

        app.cancel_uninstall_service();

        assert_eq!(app.service_modal, None);
        assert_eq!(app.status, "unchanged");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn backend_failures_land_in_the_status_line_instead_of_propagating() {
        let (mut app, root) = test_app("backend-error");
        struct FailingBackend;

        impl BackendControl for FailingBackend {
            fn status(&mut self) -> Result<BackendStatus, ControlError> {
                Ok(BackendStatus {
                    soundpack: None,
                    volume: 1.0,
                    device: None,
                    device_connected: true,
                    stream_failed: false,
                    output_device: None,
                    tone_pan: 0.0,
                    tone_distance: 0.0,
                    enabled: true,
                    modifier_sounds: true,
                    key_up_sounds: true,
                    key_up_fallback: true,
                    pitch_variation: crate::config::DEFAULT_PITCH_VARIATION,
                    velocity_variation: crate::config::DEFAULT_VELOCITY_VARIATION,
                    return_ding: false,
                })
            }

            fn set_soundpack(&mut self, _path: &Path) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn set_tone_pan(&mut self, _pan: f32) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn set_tone_distance(&mut self, _distance: f32) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn set_volume(&mut self, _volume: f32) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn set_device(&mut self, _name: &str) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn set_enabled(&mut self, _enabled: bool) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn play_sample(&mut self, _path: &Path) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn play_ding(&mut self) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn get_stats(&mut self) -> Result<crate::backend::stats::Stats, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }

            fn apply_config(&mut self, _config: &AppConfig) -> Result<BackendStatus, ControlError> {
                Err(ControlError::Refused(String::from("backend down")))
            }
        }

        let pack_root = root.join("packs");
        write_pack(&pack_root.join("cream"), "Creams");
        app.config.soundpack_roots = vec![pack_root.clone()];
        app.refresh_soundpacks().expect("refresh");
        app.backend = Some(Box::new(FailingBackend));

        app.adjust_volume(1.0).expect("volume errors are contained");

        assert!(app.status.contains("Backend unreachable"));
        assert!(!app.should_quit);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn moves_selection_in_both_directions() {
        let root = test_directory("selection");
        let config_path = root.join("config.json");
        write_pack(&root.join("alpha"), "Alpha");
        write_pack(&root.join("beta"), "Beta");
        let config = AppConfig {
            soundpack_roots: vec![root.clone()],
            ..AppConfig::default()
        };
        let mut app = App::new(config_path, config).expect("create app");

        app.select_next();
        assert_eq!(
            app.selected_pack().map(|pack| pack.name.as_str()),
            Some("Beta")
        );

        app.select_previous();
        assert_eq!(
            app.selected_pack().map(|pack| pack.name.as_str()),
            Some("Alpha")
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn service_status_check_is_throttled_to_the_poll_interval() {
        let now = Instant::now();
        let interval = Duration::from_secs(5);

        assert!(is_stale(None, now, interval));
        assert!(!is_stale(Some(now - Duration::from_secs(1)), now, interval));
        assert!(is_stale(Some(now - Duration::from_secs(6)), now, interval));
        assert!(is_stale(Some(now - interval), now, interval));
    }

    #[test]
    fn polling_the_backend_syncs_tone_values_into_the_config_the_ui_reads() {
        let (mut app, root) = test_app("tone-sync");
        app.config.tone_pan = -1.0;
        app.config.tone_distance = 0.0;
        let backend = FakeBackend {
            enabled: true,
            tone_pan: 0.4,
            tone_distance: 0.8,
            ..FakeBackend::default()
        };
        app.backend = Some(Box::new(backend));

        app.poll_process().expect("poll");

        assert_eq!(app.config.tone_pan, 0.4);
        assert_eq!(app.config.tone_distance, 0.8);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn activating_a_row_on_the_audio_tab_changes_only_that_row_and_leaves_general_settings_untouched()
     {
        let (mut app, root) = test_app("audio-row-isolation");
        let default_pitch_variation = app.config.pitch_variation;
        let default_key_up_sounds = app.config.key_up_sounds;
        app.open_settings();
        app.settings_tab = SettingsTab::Audio;
        app.settings_index = 0;

        app.activate_selected().expect("activate audio row");

        assert_ne!(app.config.pitch_variation, default_pitch_variation);
        assert_eq!(app.config.key_up_sounds, default_key_up_sounds);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn settings_tab_row_counts_match_their_tables() {
        use super::{AUDIO_SETTINGS, AUDIO_VALUE_COUNT, GENERAL_SETTING_COUNT, GENERAL_SETTINGS};

        assert_eq!(GENERAL_SETTING_COUNT, GENERAL_SETTINGS.len());
        assert_eq!(AUDIO_VALUE_COUNT, AUDIO_SETTINGS.len());
    }

    #[test]
    fn live_status_line_describes_connected_and_disconnected_states() {
        let connected = BackendStatus {
            soundpack: Some(String::from("Creams")),
            volume: 2.5,
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
        assert_eq!(live_status_line(&connected), "Creams · volume 2.5");

        let disconnected = BackendStatus {
            device_connected: false,
            ..connected.clone()
        };
        assert!(live_status_line(&disconnected).contains("no keyboard device"));

        let failed = BackendStatus {
            stream_failed: true,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 1.0,
            enabled: true,
            ..connected
        };
        assert!(live_status_line(&failed).contains("Audio stream failed"));
    }
}
