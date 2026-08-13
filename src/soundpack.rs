use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Soundpack {
    pub name: String,
    pub path: PathBuf,
    pub mapping_count: usize,
}

#[derive(Debug, Error)]
pub enum SoundpackError {
    #[error("soundpack is not a directory: {}", _0.display())]
    InvalidDirectory(PathBuf),
    #[error("soundpack config is missing: {}", _0.display())]
    MissingConfig(PathBuf),
    #[error("could not read {}: {source}", path.display())]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse {}: {source}", path.display())]
    ParseConfig {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("soundpack has no defines mapping: {}", _0.display())]
    MissingDefines(PathBuf),
    #[error("soundpack has an invalid key code '{key}': {}", path.display())]
    InvalidKey { path: PathBuf, key: String },
    #[error("soundpack references an unsafe audio path '{audio_path}': {}", path.display())]
    InvalidAudioPath { path: PathBuf, audio_path: String },
    #[error("soundpack audio file is missing: {}", _0.display())]
    MissingAudio(PathBuf),
    #[error("could not read soundpack root {}: {source}", path.display())]
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not inspect soundpack entry {}: {source}", path.display())]
    ReadEntry {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Deserialize)]
struct SoundpackConfig {
    name: Option<String>,
    defines: Option<BTreeMap<String, Option<String>>>,
    version: Option<u32>,
    key_define_type: Option<String>,
}

#[derive(Debug)]
pub struct ParsedPack {
    pub name: String,
    pub version: Option<u32>,
    pub key_define_type: Option<String>,
    pub defines: BTreeMap<u16, PathBuf>,
    pub up_defines: BTreeMap<u16, PathBuf>,
}

pub fn validate_soundpack(path: &Path) -> Result<Soundpack, SoundpackError> {
    let parsed = parse_pack(path)?;

    Ok(Soundpack {
        name: parsed.name,
        path: path.to_path_buf(),
        mapping_count: parsed.defines.len(),
    })
}

pub fn parse_pack(path: &Path) -> Result<ParsedPack, SoundpackError> {
    let config = read_config(path)?;
    let defines = config
        .defines
        .ok_or_else(|| SoundpackError::MissingDefines(path.join("config.json")))?;

    let mut down = BTreeMap::new();
    let mut up = BTreeMap::new();
    for (key, audio_path) in defines {
        let Some(audio_path) = audio_path else {
            continue;
        };

        let (key_code, is_up) = parse_defines_key(&key, path)?;
        check_audio_reference(path, &audio_path)?;
        let resolved = path.join(Path::new(&audio_path));
        if is_up {
            up.insert(key_code, resolved);
        } else {
            down.insert(key_code, resolved);
        }
    }

    let name = config
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unnamed soundpack")
                .to_string()
        });

    Ok(ParsedPack {
        name,
        version: config.version,
        key_define_type: config.key_define_type,
        defines: down,
        up_defines: up,
    })
}

pub fn discover_soundpacks(roots: &[PathBuf]) -> Result<Vec<Soundpack>, SoundpackError> {
    let root_directories = roots
        .iter()
        .filter(|root| root.is_dir())
        .map(|root| {
            fs::read_dir(root)
                .map_err(|source| SoundpackError::ReadDirectory {
                    path: root.to_path_buf(),
                    source,
                })?
                .map(|entry| {
                    entry
                        .map(|entry| entry.path())
                        .map_err(|source| SoundpackError::ReadEntry {
                            path: root.to_path_buf(),
                            source,
                        })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut directories = root_directories.into_iter().flatten().collect::<Vec<_>>();
    directories.sort();

    let valid_packs = directories
        .iter()
        .filter(|path| path.is_dir())
        .filter_map(|path| validate_soundpack(path).ok())
        .collect::<Vec<_>>();

    Ok(valid_packs)
}

fn read_config(path: &Path) -> Result<SoundpackConfig, SoundpackError> {
    if !path.is_dir() {
        return Err(SoundpackError::InvalidDirectory(path.to_path_buf()));
    }

    let config_path = path.join("config.json");
    if !config_path.is_file() {
        return Err(SoundpackError::MissingConfig(config_path.clone()));
    }

    let config_contents =
        fs::read_to_string(&config_path).map_err(|source| SoundpackError::ReadConfig {
            path: config_path.clone(),
            source,
        })?;

    serde_json::from_str(&config_contents).map_err(|source| SoundpackError::ParseConfig {
        path: config_path.clone(),
        source,
    })
}

fn parse_defines_key(key: &str, path: &Path) -> Result<(u16, bool), SoundpackError> {
    let invalid = || SoundpackError::InvalidKey {
        path: path.join("config.json"),
        key: key.to_string(),
    };

    if let Some(base) = key.strip_suffix("-up") {
        if !base.is_empty() && base.bytes().all(|byte| byte.is_ascii_digit()) {
            return base
                .parse::<u16>()
                .map(|code| (code, true))
                .map_err(|_| invalid());
        }
        return Err(invalid());
    }

    if key.contains('-') {
        return Err(invalid());
    }

    key.parse::<u16>()
        .map(|code| (code, false))
        .map_err(|_| invalid())
}

fn check_audio_reference(pack_path: &Path, audio_path: &str) -> Result<(), SoundpackError> {
    let relative_path = Path::new(audio_path);
    let has_parent = relative_path
        .components()
        .any(|component| component == Component::ParentDir);

    if relative_path.is_absolute() || has_parent {
        return Err(SoundpackError::InvalidAudioPath {
            path: pack_path.join("config.json"),
            audio_path: audio_path.to_string(),
        });
    }

    let audio_file = pack_path.join(relative_path);
    if !audio_file.is_file() {
        return Err(SoundpackError::MissingAudio(audio_file));
    }

    fs::File::open(&audio_file).map_err(|_| SoundpackError::MissingAudio(audio_file))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SoundpackError, discover_soundpacks, parse_pack, validate_soundpack};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-{name}-{}", std::process::id()))
    }

    fn write_pack(directory: &Path, config: &str, audio_file: Option<&str>) {
        fs::create_dir_all(directory).expect("create test directory");
        fs::write(directory.join("config.json"), config).expect("write test config");

        if let Some(audio_file) = audio_file {
            fs::write(directory.join(audio_file), b"audio").expect("write test audio");
        }
    }

    #[test]
    fn validates_a_pack_with_a_named_mapping() {
        let directory = test_directory("valid-pack");
        write_pack(
            &directory,
            r#"{"name":"Quiet Keys","defines":{"30":"key.wav"}}"#,
            Some("key.wav"),
        );

        let pack = validate_soundpack(&directory).expect("valid pack");

        assert_eq!(pack.name, "Quiet Keys");
        assert_eq!(pack.mapping_count, 1);
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn parses_defines_into_absolute_paths_skipping_nulls() {
        let directory = test_directory("parse-defines");
        write_pack(
            &directory,
            r#"{"name":"P","defines":{"30":"a.wav","31":null,"91":"b.wav"}}"#,
            None,
        );
        fs::write(directory.join("a.wav"), b"audio").expect("write a.wav");
        fs::write(directory.join("b.wav"), b"audio").expect("write b.wav");

        let parsed = parse_pack(&directory).expect("parse pack");

        assert_eq!(parsed.defines.len(), 2);
        assert_eq!(parsed.defines.get(&30), Some(&directory.join("a.wav")));
        assert_eq!(parsed.defines.get(&91), Some(&directory.join("b.wav")));
        assert!(!parsed.defines.contains_key(&31));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn accepts_valid_up_entries_and_rejects_malformed_keys() {
        let directory = test_directory("keyup-ok");
        write_pack(
            &directory,
            r#"{"defines":{"30":"a.wav","30-up":"a-up.wav"}}"#,
            None,
        );
        fs::write(directory.join("a.wav"), b"audio").expect("write a.wav");
        fs::write(directory.join("a-up.wav"), b"audio").expect("write a-up.wav");

        let parsed = parse_pack(&directory).expect("valid up entry parses");
        assert_eq!(parsed.defines.get(&30), Some(&directory.join("a.wav")));
        assert_eq!(
            parsed.up_defines.get(&30),
            Some(&directory.join("a-up.wav"))
        );
        fs::remove_dir_all(directory).expect("remove test directory");

        let malformed = test_directory("keyup-bad");
        write_pack(
            &malformed,
            r#"{"defines":{"1-upx":"b.wav"}}"#,
            Some("b.wav"),
        );
        let error = parse_pack(&malformed).expect_err("malformed key should fail");
        assert!(matches!(error, SoundpackError::InvalidKey { .. }));
        fs::remove_dir_all(malformed).expect("remove test directory");
    }

    #[test]
    fn surfaces_version_and_key_define_type_for_warnings() {
        let directory = test_directory("metadata");
        write_pack(
            &directory,
            r#"{"name":"V2","version":2,"key_define_type":"multiple","defines":{"30":"key.wav"}}"#,
            Some("key.wav"),
        );

        let parsed = parse_pack(&directory).expect("parse pack");

        assert_eq!(parsed.version, Some(2));
        assert_eq!(parsed.key_define_type.as_deref(), Some("multiple"));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn rejects_a_pack_with_a_missing_audio_file() {
        let directory = test_directory("missing-audio");
        write_pack(&directory, r#"{"defines":{"30":"missing.wav"}}"#, None);

        let error = validate_soundpack(&directory).expect_err("missing audio should fail");

        assert!(error.to_string().contains("missing.wav"));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn discovers_valid_packs_from_a_root() {
        let root = test_directory("discovery");
        write_pack(
            &root.join("alpha"),
            r#"{"name":"Alpha","defines":{"30":"key.wav"}}"#,
            Some("key.wav"),
        );
        write_pack(
            &root.join("beta"),
            r#"{"name":"Beta","defines":{"31":"key.wav"}}"#,
            Some("key.wav"),
        );

        let packs = discover_soundpacks(std::slice::from_ref(&root)).expect("discover packs");

        assert_eq!(
            packs
                .iter()
                .map(|pack| pack.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Beta"]
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
