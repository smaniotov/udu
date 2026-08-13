use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

pub const MIN_VOLUME: f32 = 0.0;
pub const MAX_VOLUME: f32 = 100.0;
pub const VOLUME_SOFT: f32 = 30.0;
pub const VOLUME_BALANCED: f32 = 60.0;
pub const VOLUME_LOUD: f32 = 90.0;

pub const LEGACY_OLD_SCALE_MAX: f32 = 10.0;
pub const CURRENT_VOLUME_SCALE_VERSION: u32 = 1;

pub fn clamp_volume(volume: f32) -> f32 {
    volume.clamp(MIN_VOLUME, MAX_VOLUME)
}

pub fn migrate_volume_scale(config_path: &Path) -> Result<(), ConfigError> {
    if !config_path.is_file() {
        return Ok(());
    }

    let contents = fs::read_to_string(config_path).map_err(|source| ConfigError::Read {
        path: config_path.to_path_buf(),
        source,
    })?;
    let mut value: serde_json::Value =
        serde_json::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: config_path.to_path_buf(),
            source,
        })?;

    let scale_version = value
        .get("volume_scale_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    if scale_version >= u64::from(CURRENT_VOLUME_SCALE_VERSION) {
        return Ok(());
    }

    let Some(volume) = value.get_mut("volume") else {
        return Ok(());
    };
    let Some(current) = volume.as_f64() else {
        return Ok(());
    };

    if current <= f64::from(LEGACY_OLD_SCALE_MAX) {
        *volume = serde_json::Value::from(current * 10.0);
    }

    value["volume_scale_version"] = serde_json::Value::from(CURRENT_VOLUME_SCALE_VERSION);
    let migrated =
        serde_json::to_string_pretty(&value).map_err(|source| ConfigError::Serialize { source })?;

    write_config_atomically(config_path, &migrated)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub soundpack_roots: Vec<PathBuf>,
    pub selected_soundpack: Option<PathBuf>,
    pub volume: f32,
    #[serde(default)]
    pub volume_scale_version: u32,
    pub device_name: Option<String>,
    #[serde(default = "default_true")]
    pub modifier_sounds: bool,
    #[serde(default = "default_true")]
    pub key_up_sounds: bool,
    #[serde(default = "default_true")]
    pub key_up_fallback: bool,
    #[serde(default = "default_pitch_variation")]
    pub pitch_variation: f32,
    #[serde(default = "default_velocity_variation")]
    pub velocity_variation: f32,
    #[serde(default)]
    pub return_ding: bool,
    #[serde(default)]
    pub output_device: Option<String>,
    #[serde(default)]
    pub tone_pan: f32,
    #[serde(default)]
    pub tone_distance: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            soundpack_roots: Vec::new(),
            selected_soundpack: None,
            volume: 10.0,
            volume_scale_version: CURRENT_VOLUME_SCALE_VERSION,
            device_name: None,
            modifier_sounds: true,
            key_up_sounds: true,
            key_up_fallback: true,
            pitch_variation: DEFAULT_PITCH_VARIATION,
            velocity_variation: DEFAULT_VELOCITY_VARIATION,
            return_ding: false,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 0.0,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse config {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("could not serialize config: {source}")]
    Serialize { source: serde_json::Error },
    #[error("could not create config directory {}: {source}", path.display())]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write config {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not migrate legacy data: {source}")]
    Migrate { source: std::io::Error },
}

pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    if !path.is_file() {
        return Ok(AppConfig::default());
    }

    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

const LEGACY_APP_DIR: &str = "wayvibes-tui";
pub const APP_DIR: &str = "udu";
pub const DEFAULT_PITCH_VARIATION: f32 = 0.06;
pub const DEFAULT_VELOCITY_VARIATION: f32 = 0.15;
pub const MAX_VARIATION: f32 = 0.5;

fn default_true() -> bool {
    true
}

fn default_pitch_variation() -> f32 {
    DEFAULT_PITCH_VARIATION
}

fn default_velocity_variation() -> f32 {
    DEFAULT_VELOCITY_VARIATION
}

pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|directory| directory.join(APP_DIR).join("config.json"))
}

pub fn migrate_legacy_dirs() -> Result<(), ConfigError> {
    let (Some(config_dir), Some(data_dir)) = (dirs::config_dir(), dirs::data_dir()) else {
        return Ok(());
    };

    migrate_dirs(
        &config_dir.join(LEGACY_APP_DIR),
        &data_dir.join(LEGACY_APP_DIR),
        &config_dir.join(APP_DIR),
        &data_dir.join(APP_DIR),
    )
}

fn migrate_dirs(
    old_config: &Path,
    old_data: &Path,
    new_config: &Path,
    new_data: &Path,
) -> Result<(), ConfigError> {
    let new_config_file = new_config.join("config.json");

    if new_config_file.exists() {
        return Ok(());
    }

    let old_config_file = old_config.join("config.json");
    if !old_config_file.exists() {
        return Ok(());
    }

    let old_packs = old_data.join("soundpacks");
    if old_packs.is_dir() {
        let new_packs = new_data.join("soundpacks");
        if !new_packs.exists() {
            fs::create_dir_all(&new_packs).map_err(|source| ConfigError::CreateDirectory {
                path: new_packs.clone(),
                source,
            })?;
            copy_tree(&old_packs, &new_packs).map_err(|source| ConfigError::Migrate { source })?;
        }
    }

    let contents = fs::read_to_string(&old_config_file).map_err(|source| ConfigError::Read {
        path: old_config_file.clone(),
        source,
    })?;
    let mut value: serde_json::Value =
        serde_json::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: old_config_file.clone(),
            source,
        })?;
    rewrite_old_data_paths(&mut value, old_data, new_data);
    let migrated =
        serde_json::to_string_pretty(&value).map_err(|source| ConfigError::Serialize { source })?;

    fs::create_dir_all(new_config).map_err(|source| ConfigError::CreateDirectory {
        path: new_config.to_path_buf(),
        source,
    })?;
    fs::write(&new_config_file, migrated).map_err(|source| ConfigError::Write {
        path: new_config_file.clone(),
        source,
    })?;

    Ok(())
}

fn rewrite_old_data_paths(value: &mut serde_json::Value, old_data: &Path, new_data: &Path) {
    match value {
        serde_json::Value::String(text) => {
            let candidate = Path::new(text.as_str());
            if candidate.starts_with(old_data)
                && let Ok(relative) = candidate.strip_prefix(old_data)
            {
                *text = new_data.join(relative).to_string_lossy().into_owned();
            }
        }
        serde_json::Value::Array(entries) => {
            for entry in entries {
                rewrite_old_data_paths(entry, old_data, new_data);
            }
        }
        serde_json::Value::Object(fields) => {
            for field in fields.values_mut() {
                rewrite_old_data_paths(field, old_data, new_data);
            }
        }
        _ => {}
    }
}

pub fn copy_tree(source: &Path, target: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn prepare_config(config: AppConfig) -> Result<AppConfig, ConfigError> {
    let config = AppConfig {
        volume: clamp_volume(config.volume),
        ..config
    };

    if !config.soundpack_roots.is_empty() {
        return Ok(config);
    }

    let Some(soundpack_root) = default_soundpack_root() else {
        return Ok(config);
    };

    fs::create_dir_all(&soundpack_root).map_err(|source| ConfigError::CreateDirectory {
        path: soundpack_root.clone(),
        source,
    })?;

    let mut soundpack_roots = vec![soundpack_root];
    if let Some(local_root) = local_soundpack_root() {
        soundpack_roots.push(local_root);
    }

    Ok(AppConfig {
        soundpack_roots,
        ..config
    })
}

pub fn default_soundpack_root() -> Option<PathBuf> {
    dirs::data_dir().map(|directory| directory.join("udu").join("soundpacks"))
}

fn local_soundpack_root() -> Option<PathBuf> {
    let root = std::env::current_dir().ok()?.join("sounds");

    root.is_dir().then(|| fs::canonicalize(root).ok()).flatten()
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    let contents =
        serde_json::to_string_pretty(config).map_err(|source| ConfigError::Serialize { source })?;

    write_config_atomically(path, &format!("{contents}\n"))
}

fn write_config_atomically(path: &Path, contents: &str) -> Result<(), ConfigError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let temp_path = temp_sibling(path);
    if let Err(source) = write_new_file(&temp_path, contents) {
        let _ = fs::remove_file(&temp_path);

        return Err(ConfigError::Write {
            path: temp_path,
            source,
        });
    }

    if let Err(source) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);

        return Err(ConfigError::Write {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}

fn write_new_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;

    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

static TEMP_SIBLING_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_sibling(path: &Path) -> PathBuf {
    let unique = TEMP_SIBLING_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.as_os_str().to_os_string();

    name.push(format!(".tmp.{}.{unique}", std::process::id()));

    PathBuf::from(name)
}

#[cfg(test)]
mod migration_tests {
    use super::{copy_tree, migrate_dirs, rewrite_old_data_paths};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-migrate-{name}-{}", std::process::id()))
    }

    fn write_pack(root: &Path, pack: &str) {
        let dir = root.join(pack);
        fs::create_dir_all(&dir).expect("create pack dir");
        fs::write(dir.join("config.json"), r#"{"defines":{"30":"key.wav"}}"#).expect("config");
        fs::write(dir.join("key.wav"), b"audio").expect("key.wav");
    }

    #[test]
    fn migrates_config_and_packs_into_the_new_dirs_rewriting_paths() {
        let root = test_dir("basic");
        let old_config = root.join("old-config");
        let old_data = root.join("old-data");
        let new_config = root.join("new-config");
        let new_data = root.join("new-data");
        write_pack(&old_data.join("soundpacks"), "cream");
        write_pack(&old_data.join("soundpacks"), "oreo");
        fs::create_dir_all(&old_config).expect("old config dir");
        fs::write(
            old_config.join("config.json"),
            format!(
                r#"{{"soundpack_roots":["{old}/soundpacks"],"selected_soundpack":"{old}/soundpacks/cream","volume":9.0,"device_name":null}}"#,
                old = old_data.display()
            ),
        )
        .expect("old config");

        migrate_dirs(&old_config, &old_data, &new_config, &new_data).expect("migrate");

        let migrated: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(new_config.join("config.json")).expect("new config"),
        )
        .expect("parse");
        assert_eq!(
            migrated["soundpack_roots"][0].as_str().unwrap(),
            new_data.join("soundpacks").to_str().unwrap()
        );
        assert_eq!(
            migrated["selected_soundpack"].as_str().unwrap(),
            new_data.join("soundpacks/cream").to_str().unwrap()
        );
        assert_eq!(migrated["volume"].as_f64(), Some(9.0));
        assert!(new_data.join("soundpacks/cream/config.json").is_file());
        assert!(new_data.join("soundpacks/oreo/").is_dir());
        assert!(old_data.join("soundpacks").is_dir(), "old data kept");
        assert!(old_config.join("config.json").is_file(), "old config kept");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn does_not_overwrite_an_existing_new_config() {
        let root = test_dir("nooverwrite");
        let old_config = root.join("old-config");
        let old_data = root.join("old-data");
        let new_config = root.join("new-config");
        let new_data = root.join("new-data");
        fs::create_dir_all(&old_config).expect("old dir");
        fs::write(old_config.join("config.json"), r#"{"volume":1.0}"#).expect("old config");
        fs::create_dir_all(&new_config).expect("new dir");
        fs::write(new_config.join("config.json"), r#"{"volume":5.0}"#).expect("new config");

        migrate_dirs(&old_config, &old_data, &new_config, &new_data).expect("migrate");

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(new_config.join("config.json")).unwrap())
                .unwrap();
        assert_eq!(value["volume"].as_f64(), Some(5.0));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rewrites_only_strings_under_the_old_data_root() {
        let old = Path::new("/old/data");
        let new = Path::new("/new/data");
        let mut value: serde_json::Value = serde_json::from_str(
            r#"{"a":"/old/data/packs/x","b":"/elsewhere/y","c":[{"d":"/old/data/packs/z"}],"e":42}"#,
        )
        .unwrap();

        rewrite_old_data_paths(&mut value, old, new);

        assert_eq!(value["a"].as_str().unwrap(), "/new/data/packs/x");
        assert_eq!(value["b"].as_str().unwrap(), "/elsewhere/y");
        assert_eq!(value["c"][0]["d"].as_str().unwrap(), "/new/data/packs/z");
        assert_eq!(value["e"].as_u64(), Some(42));
    }

    #[test]
    fn copy_tree_replicates_files_and_subdirectories() {
        let root = test_dir("copytree");
        let src = root.join("src");
        let dst = root.join("dst");
        write_pack(&src, "a");
        fs::create_dir_all(src.join("nested/deep")).expect("nested");
        fs::write(src.join("nested/deep/x.wav"), b"x").expect("x");

        copy_tree(&src, &dst).expect("copy");

        assert!(dst.join("a/config.json").is_file());
        assert!(dst.join("nested/deep/x.wav").is_file());
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, CURRENT_VOLUME_SCALE_VERSION, DEFAULT_PITCH_VARIATION,
        DEFAULT_VELOCITY_VARIATION, clamp_volume, default_soundpack_root, load_config,
        migrate_volume_scale, prepare_config, save_config,
    };
    use std::fs;
    use std::path::PathBuf;

    fn test_config_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-config-{name}-{}", std::process::id()))
    }

    #[test]
    fn clamp_volume_bounds_to_the_valid_range() {
        use super::{MAX_VOLUME, MIN_VOLUME};

        assert_eq!(clamp_volume(-1.0), MIN_VOLUME);
        assert_eq!(clamp_volume(200.0), MAX_VOLUME);
        assert_eq!(clamp_volume(50.0), 50.0);
    }

    #[test]
    fn atomic_save_leaves_no_temp_sibling() {
        let root = test_config_path("atomic");
        let path = root.join("config.json");

        save_config(&path, &AppConfig::default()).expect("save config");

        assert!(path.is_file());
        let leftover_temp_files = fs::read_dir(&root)
            .expect("read test directory")
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_name().to_string_lossy().contains(".tmp."));
        assert!(
            !leftover_temp_files,
            "no temp sibling should remain after an atomic save"
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn volume_scale_migration_multiplies_legacy_values_once() {
        let cases = [(1.5, 15.0), (0.8, 8.0), (10.0, 100.0), (42.0, 42.0)];

        for (index, (input, expected)) in cases.into_iter().enumerate() {
            let path = test_config_path(&format!("volscale-table-{index}"));
            fs::write(&path, format!(r#"{{"volume":{input}}}"#)).expect("write");

            migrate_volume_scale(&path).expect("first migrate");
            let value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
            assert_eq!(value["volume"].as_f64(), Some(expected), "input {input}");

            migrate_volume_scale(&path).expect("second migrate must be a no-op");
            let value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
            assert_eq!(
                value["volume"].as_f64(),
                Some(expected),
                "second migration must not change input {input}"
            );

            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn volume_scale_migration_leaves_new_scale_and_missing_volume_untouched() {
        let path = test_config_path("volscale-new");
        fs::write(&path, r#"{"volume":42.0}"#).expect("write");
        migrate_volume_scale(&path).expect("migrate");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(value["volume"].as_f64(), Some(42.0));

        let no_volume = test_config_path("volscale-none");
        fs::write(&no_volume, r#"{"device_name":"kbd"}"#).expect("write");
        migrate_volume_scale(&no_volume).expect("migrate");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&no_volume).expect("read")).expect("parse");
        assert_eq!(value["device_name"].as_str(), Some("kbd"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&no_volume);
    }

    #[test]
    fn legacy_configs_default_modifier_sounds_to_enabled() {
        let path = test_config_path("legacy-defaults");
        fs::write(&path, r#"{"soundpack_roots":["/sounds"],"volume":2.0}"#).expect("write");

        let config = load_config(&path).expect("load");

        assert!(config.modifier_sounds);
        assert!(config.key_up_sounds);
        assert!(config.key_up_fallback);
        assert!(!config.return_ding);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn saves_and_loads_selected_pack_and_volume() {
        let path = test_config_path("round-trip");
        let config = AppConfig {
            soundpack_roots: vec![PathBuf::from("/sounds")],
            selected_soundpack: Some(PathBuf::from("/sounds/quiet")),
            volume: 3.5,
            volume_scale_version: CURRENT_VOLUME_SCALE_VERSION,
            device_name: Some(String::from("USB Keyboard")),
            modifier_sounds: true,
            key_up_sounds: true,
            key_up_fallback: true,
            pitch_variation: DEFAULT_PITCH_VARIATION,
            velocity_variation: DEFAULT_VELOCITY_VARIATION,
            return_ding: false,
            output_device: None,
            tone_pan: 0.0,
            tone_distance: 0.0,
        };

        save_config(&path, &config).expect("save config");
        let loaded = load_config(&path).expect("load config");

        assert_eq!(loaded, config);
        fs::remove_file(path).expect("remove test config");
    }

    #[test]
    fn creates_parent_directories_when_saving_a_new_config() {
        let root = test_config_path("nested");
        let path = root.join("new/config.json");

        save_config(&path, &AppConfig::default()).expect("save nested config");

        assert!(path.is_file());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn adds_the_user_soundpack_root_when_no_root_is_configured() {
        let config = prepare_config(AppConfig::default()).expect("prepare default config");
        let root = default_soundpack_root().expect("user data directory");

        assert_eq!(config.soundpack_roots.first(), Some(&root));
    }

    #[test]
    fn keeps_explicit_soundpack_roots_unchanged() {
        let config = AppConfig {
            soundpack_roots: vec![PathBuf::from("/custom/soundpacks")],
            ..AppConfig::default()
        };

        let prepared = prepare_config(config.clone()).expect("prepare explicit config");

        assert_eq!(prepared, config);
    }

    #[test]
    fn missing_config_uses_defaults() {
        let path = test_config_path("defaults");

        let config = load_config(&path).expect("load default config");

        assert_eq!(config, AppConfig::default());
    }
}
