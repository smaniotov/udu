use evdev::{Device, KeyCode};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const INPUT_DIRECTORY: &str = "/dev/input";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardDevice {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("could not inspect {}: {source}", path.display())]
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not inspect an input entry in {}: {source}", path.display())]
    ReadEntry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not read input device {}: {source}", path.display())]
    OpenDevice {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("input node is not a keyboard: {}", _0.display())]
    NotKeyboard(PathBuf),
}

pub fn discover_keyboards() -> Result<Vec<KeyboardDevice>, DeviceError> {
    let paths = fs::read_dir(INPUT_DIRECTORY)
        .map_err(|source| DeviceError::ReadDirectory {
            path: PathBuf::from(INPUT_DIRECTORY),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| DeviceError::ReadEntry {
                    path: PathBuf::from(INPUT_DIRECTORY),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let classified = paths
        .into_iter()
        .filter(|path| is_event_device(path))
        .map(|path| classify_event_node(&path));

    Ok(collect_keyboards(classified))
}

fn classify_event_node(path: &Path) -> Result<KeyboardDevice, DeviceError> {
    let device = Device::open(path).map_err(|source| DeviceError::OpenDevice {
        path: path.to_path_buf(),
        source,
    })?;
    let supported_keys = device.supported_keys();

    if !supported_keys.is_some_and(|keys| keys.contains(KeyCode::KEY_A)) {
        return Err(DeviceError::NotKeyboard(path.to_path_buf()));
    }

    Ok(KeyboardDevice {
        name: device.name().unwrap_or("Unnamed keyboard").to_string(),
        path: path.to_path_buf(),
    })
}

fn collect_keyboards(
    entries: impl Iterator<Item = Result<KeyboardDevice, DeviceError>>,
) -> Vec<KeyboardDevice> {
    let mut devices: Vec<KeyboardDevice> = entries.filter_map(Result::ok).collect();

    devices.sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));

    devices
}

fn is_event_device(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("event"))
}

#[cfg(test)]
mod tests {
    use super::{DeviceError, KeyboardDevice, collect_keyboards, is_event_device};
    use std::path::{Path, PathBuf};

    #[test]
    fn classifies_event_nodes_without_opening_them() {
        assert!(is_event_device(Path::new("/dev/input/event17")));
        assert!(!is_event_device(Path::new("/dev/input/mouse0")));
    }

    #[test]
    fn skips_unreadable_and_non_keyboard_nodes() {
        let keyboard = KeyboardDevice {
            name: String::from("K"),
            path: PathBuf::from("/dev/input/event0"),
        };
        let entries = vec![
            Ok(keyboard.clone()),
            Err(DeviceError::NotKeyboard(PathBuf::from("/dev/input/event1"))),
            Err(DeviceError::OpenDevice {
                path: PathBuf::from("/dev/input/event2"),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            }),
            Ok(keyboard.clone()),
        ];

        let devices = collect_keyboards(entries.into_iter());

        assert_eq!(devices, vec![keyboard.clone(), keyboard]);
    }
}
