use crate::soundpack::{self, ParsedPack, SoundpackError};
use evdev::KeyCode;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MappingError {
    #[error("{0}")]
    Soundpack(#[from] SoundpackError),
}

#[derive(Debug)]
pub struct Mapping {
    pub pack_name: String,
    pub pack_path: PathBuf,
    pub warnings: Vec<String>,
    by_down: BTreeMap<u16, PathBuf>,
    by_up: BTreeMap<u16, PathBuf>,
}

impl Mapping {
    pub fn load(pack_path: &Path) -> Result<Self, MappingError> {
        let parsed = soundpack::parse_pack(pack_path)?;
        let ParsedPack {
            name,
            version,
            key_define_type,
            defines,
            up_defines,
        } = parsed;
        let by_down = defines
            .into_iter()
            .map(|(code, path)| (iohook_to_evdev(code), path))
            .collect();
        let by_up = up_defines
            .into_iter()
            .map(|(code, path)| (iohook_to_evdev(code), path))
            .collect();

        Ok(Self {
            pack_name: name,
            pack_path: pack_path.to_path_buf(),
            warnings: build_warnings(version, key_define_type.as_deref()),
            by_down,
            by_up,
        })
    }

    pub fn lookup_down(&self, evdev_code: u16) -> Option<&Path> {
        self.by_down.get(&evdev_code).map(PathBuf::as_path)
    }

    pub fn lookup_up(&self, evdev_code: u16) -> Option<&Path> {
        self.by_up.get(&evdev_code).map(PathBuf::as_path)
    }
}

pub const fn iohook_to_evdev(code: u16) -> u16 {
    match code {
        57416 | 61000 => KeyCode::KEY_UP.code(),
        57424 | 61008 => KeyCode::KEY_DOWN.code(),
        57419 | 61003 => KeyCode::KEY_LEFT.code(),
        57421 | 61005 => KeyCode::KEY_RIGHT.code(),
        3655 | 60999 => KeyCode::KEY_HOME.code(),
        3663 | 61007 => KeyCode::KEY_END.code(),
        3657 | 61001 => KeyCode::KEY_PAGEUP.code(),
        3665 | 61009 => KeyCode::KEY_PAGEDOWN.code(),
        3666 | 61010 => KeyCode::KEY_INSERT.code(),
        3667 | 61011 => KeyCode::KEY_DELETE.code(),
        3612 => KeyCode::KEY_KPENTER.code(),
        3637 => KeyCode::KEY_KPSLASH.code(),
        3597 => KeyCode::KEY_KPEQUAL.code(),
        3639 => KeyCode::KEY_SYSRQ.code(),
        3653 => KeyCode::KEY_PAUSE.code(),
        91 => KeyCode::KEY_F13.code(),
        92 => KeyCode::KEY_F14.code(),
        93 => KeyCode::KEY_F15.code(),
        3613 => KeyCode::KEY_RIGHTCTRL.code(),
        3640 => KeyCode::KEY_RIGHTALT.code(),
        3675 => KeyCode::KEY_LEFTMETA.code(),
        3676 => KeyCode::KEY_RIGHTMETA.code(),
        3677 => KeyCode::KEY_MENU.code(),
        _ => code,
    }
}

fn build_warnings(version: Option<u32>, key_define_type: Option<&str>) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Some(version) = version.filter(|version| *version != 1) {
        warnings.push(format!(
            "pack version {version} is not v1: v2/v3 fallback fields are not honored"
        ));
    }

    if let Some(kind) = key_define_type.filter(|kind| *kind != "multi" && *kind != "single") {
        warnings.push(format!("key_define_type '{kind}' is not 'multi'/'single'"));
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::{Mapping, build_warnings, iohook_to_evdev};
    use evdev::KeyCode;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("udu-map-{name}-{}", std::process::id()))
    }

    fn write_pack(directory: &Path, config: &str, audio_files: &[&str]) {
        fs::create_dir_all(directory).expect("create test directory");
        fs::write(directory.join("config.json"), config).expect("write test config");
        for file in audio_files {
            fs::write(directory.join(file), b"audio").expect("write test audio");
        }
    }

    #[test]
    fn maps_every_iohook_table_entry_to_its_evdev_code() {
        let cases = [
            (57416, KeyCode::KEY_UP.code()),
            (57424, KeyCode::KEY_DOWN.code()),
            (57419, KeyCode::KEY_LEFT.code()),
            (57421, KeyCode::KEY_RIGHT.code()),
            (61000, KeyCode::KEY_UP.code()),
            (61008, KeyCode::KEY_DOWN.code()),
            (61003, KeyCode::KEY_LEFT.code()),
            (61005, KeyCode::KEY_RIGHT.code()),
            (3655, KeyCode::KEY_HOME.code()),
            (60999, KeyCode::KEY_HOME.code()),
            (3663, KeyCode::KEY_END.code()),
            (61007, KeyCode::KEY_END.code()),
            (3657, KeyCode::KEY_PAGEUP.code()),
            (61001, KeyCode::KEY_PAGEUP.code()),
            (3665, KeyCode::KEY_PAGEDOWN.code()),
            (61009, KeyCode::KEY_PAGEDOWN.code()),
            (3666, KeyCode::KEY_INSERT.code()),
            (61010, KeyCode::KEY_INSERT.code()),
            (3667, KeyCode::KEY_DELETE.code()),
            (61011, KeyCode::KEY_DELETE.code()),
            (3612, KeyCode::KEY_KPENTER.code()),
            (3637, KeyCode::KEY_KPSLASH.code()),
            (3597, KeyCode::KEY_KPEQUAL.code()),
            (3639, KeyCode::KEY_SYSRQ.code()),
            (3653, KeyCode::KEY_PAUSE.code()),
            (91, KeyCode::KEY_F13.code()),
            (92, KeyCode::KEY_F14.code()),
            (93, KeyCode::KEY_F15.code()),
            (3613, KeyCode::KEY_RIGHTCTRL.code()),
            (3640, KeyCode::KEY_RIGHTALT.code()),
            (3675, KeyCode::KEY_LEFTMETA.code()),
            (3676, KeyCode::KEY_RIGHTMETA.code()),
            (3677, KeyCode::KEY_MENU.code()),
        ];

        for (iohook, evdev) in cases {
            assert_eq!(
                iohook_to_evdev(iohook),
                evdev,
                "iohook {iohook} should map to evdev {evdev}"
            );
        }
    }

    #[test]
    fn end_and_pagedown_use_the_corrected_direction() {
        assert_eq!(iohook_to_evdev(3663), KeyCode::KEY_END.code());
        assert_eq!(iohook_to_evdev(3665), KeyCode::KEY_PAGEDOWN.code());
        assert_ne!(iohook_to_evdev(3663), KeyCode::KEY_PAGEDOWN.code());
        assert_ne!(iohook_to_evdev(3665), KeyCode::KEY_END.code());
    }

    #[test]
    fn base_block_passes_through_including_right_shift() {
        assert_eq!(iohook_to_evdev(30), 30);
        assert_eq!(iohook_to_evdev(54), 54);
        assert_eq!(iohook_to_evdev(62), 62);
        assert_eq!(iohook_to_evdev(1), 1);
        assert_eq!(iohook_to_evdev(88), 88);
    }

    #[test]
    fn high_special_codes_pass_through_unless_in_the_table() {
        assert_eq!(iohook_to_evdev(61011), KeyCode::KEY_DELETE.code());
        assert_eq!(iohook_to_evdev(12345), 12345);
        assert_eq!(iohook_to_evdev(9999), 9999);
    }

    #[test]
    fn loads_a_pack_and_looks_up_by_evdev_code() {
        let directory = test_directory("lookup");
        write_pack(
            &directory,
            r#"{"name":"M","defines":{"30":"a.wav","57416":"b.wav","91":null,"3663":"e.wav"}}"#,
            &["a.wav", "b.wav", "e.wav"],
        );

        let mapping = Mapping::load(&directory).expect("load mapping");

        assert_eq!(mapping.pack_name, "M");
        assert_eq!(
            mapping.lookup_down(30),
            Some(directory.join("a.wav").as_path())
        );
        assert_eq!(
            mapping.lookup_down(KeyCode::KEY_UP.code()),
            Some(directory.join("b.wav").as_path())
        );
        assert_eq!(
            mapping.lookup_down(KeyCode::KEY_END.code()),
            Some(directory.join("e.wav").as_path())
        );
        assert!(mapping.lookup_down(KeyCode::KEY_F13.code()).is_none());
        assert!(mapping.lookup_down(88).is_none());
        assert!(mapping.warnings.is_empty());
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn warns_on_v2_version_and_unusual_key_define_type() {
        let directory = test_directory("warnings");
        write_pack(
            &directory,
            r#"{"name":"W","version":2,"key_define_type":"multiple","defines":{"30":"a.wav"}}"#,
            &["a.wav"],
        );

        let mapping = Mapping::load(&directory).expect("load mapping");

        assert_eq!(mapping.warnings.len(), 2);
        assert!(mapping.warnings[0].contains("version 2"));
        assert!(mapping.warnings[1].contains("multiple"));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn warnings_helper_is_empty_for_a_v1_multi_pack() {
        assert!(build_warnings(None, Some("multi")).is_empty());
    }

    #[test]
    fn load_accepts_optional_up_entries() {
        let directory = test_directory("keyup");
        write_pack(&directory, r#"{"defines":{"14-up":"a.mp3"}}"#, &["a.mp3"]);

        let mapping = Mapping::load(&directory).expect("pack with an up entry loads");
        assert_eq!(
            mapping.lookup_up(14),
            Some(directory.join("a.mp3").as_path())
        );
        assert!(mapping.lookup_down(14).is_none());
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn parses_a_real_world_shaped_pack() {
        let directory = test_directory("real");
        write_pack(
            &directory,
            r#"{"name":"Creams","key_define_type":"multiple","defines":{"30":"30.wav","3613":"ctrl.wav","57416":"up.wav","91":null}}"#,
            &["30.wav", "ctrl.wav", "up.wav"],
        );

        let mapping = Mapping::load(&directory).expect("load mapping");

        assert_eq!(
            mapping.lookup_down(30),
            Some(directory.join("30.wav").as_path())
        );
        assert_eq!(
            mapping.lookup_down(KeyCode::KEY_UP.code()),
            Some(directory.join("up.wav").as_path())
        );
        assert_eq!(
            mapping.lookup_down(KeyCode::KEY_RIGHTCTRL.code()),
            Some(directory.join("ctrl.wav").as_path())
        );
        assert_eq!(mapping.warnings.len(), 1);
        assert!(mapping.warnings[0].contains("multiple"));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn parse_pack_fixture_keeps_iohook_keys() {
        let directory = test_directory("raw");
        write_pack(&directory, r#"{"defines":{"30":"a.wav"}}"#, &["a.wav"]);

        let parsed = crate::soundpack::parse_pack(&directory).expect("parse pack");
        assert!(parsed.defines.contains_key(&30));
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
