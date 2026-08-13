use crate::device::{DeviceError, KeyboardDevice, discover_keyboards};
use evdev::{Device, EventType, InputEvent};
use std::collections::VecDeque;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use thiserror::Error;

const POLL_TIMEOUT_MS: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: u16,
    pub kind: KeyEventKind,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("could not inspect keyboard devices: {0}")]
    Discovery(#[from] DeviceError),
    #[error("no keyboard named '{name}' was found under /dev/input")]
    DeviceNotFound { name: String },
    #[error("could not open keyboard device {}: {source}", path.display())]
    OpenDevice { path: PathBuf, source: io::Error },
    #[error("could not poll keyboard device {}: {source}", path.display())]
    Poll { path: PathBuf, source: io::Error },
    #[error("could not read keyboard device {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("keyboard device {} was disconnected", path.display())]
    DeviceGone { path: PathBuf },
}

pub trait KeyEventSource {
    fn next_key_event(&mut self) -> Result<Option<KeyEvent>, CaptureError>;
}

pub struct Capture {
    device: Device,
    path: PathBuf,
    name: String,
    pending: VecDeque<KeyEvent>,
}

impl KeyEventSource for Capture {
    fn next_key_event(&mut self) -> Result<Option<KeyEvent>, CaptureError> {
        self.next_key_event()
    }
}

impl Capture {
    pub fn open(device_name: &str) -> Result<Self, CaptureError> {
        let keyboard = resolve_keyboard(device_name)?;
        let (device, path) = open_keyboard(&keyboard)?;

        Ok(Self {
            device,
            path,
            name: keyboard.name.clone(),
            pending: VecDeque::new(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn next_key_event(&mut self) -> Result<Option<KeyEvent>, CaptureError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }

        if !wait_readable(&self.device, &self.path)? {
            return Ok(None);
        }

        match self.device.fetch_events() {
            Ok(events) => {
                push_events(events, &mut self.pending);
                Ok(self.pending.pop_front())
            }
            Err(source) if is_device_gone(&source) => Err(CaptureError::DeviceGone {
                path: self.path.clone(),
            }),
            Err(source) if source.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(source) if is_interrupted(&source) => Ok(None),
            Err(source) => Err(CaptureError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }

    pub fn reconnect(&mut self) -> Result<(), CaptureError> {
        let keyboard = resolve_keyboard(&self.name)?;
        let (device, path) = open_keyboard(&keyboard)?;

        self.device = device;
        self.path = path;
        self.pending.clear();

        Ok(())
    }
}

fn resolve_keyboard(device_name: &str) -> Result<KeyboardDevice, CaptureError> {
    let keyboards = discover_keyboards()?;
    let keyboard = keyboards
        .into_iter()
        .find(|keyboard| keyboard.name == device_name)
        .ok_or_else(|| CaptureError::DeviceNotFound {
            name: device_name.to_string(),
        })?;

    Ok(keyboard)
}

fn open_keyboard(keyboard: &KeyboardDevice) -> Result<(Device, PathBuf), CaptureError> {
    let device = Device::open(&keyboard.path).map_err(|source| CaptureError::OpenDevice {
        path: keyboard.path.clone(),
        source,
    })?;

    device
        .set_nonblocking(true)
        .map_err(|source| CaptureError::OpenDevice {
            path: keyboard.path.clone(),
            source,
        })?;

    Ok((device, keyboard.path.clone()))
}

fn wait_readable(device: &Device, path: &Path) -> Result<bool, CaptureError> {
    let mut fds = [libc::pollfd {
        fd: device.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];

    let ready = loop {
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, POLL_TIMEOUT_MS) };

        if ready >= 0 {
            break ready;
        }

        let source = io::Error::last_os_error();

        if is_interrupted(&source) {
            continue;
        }

        return Err(CaptureError::Poll {
            path: path.to_path_buf(),
            source,
        });
    };

    if ready == 0 {
        return Ok(false);
    }

    let revents = fds[0].revents;

    if revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
        return Err(CaptureError::DeviceGone {
            path: path.to_path_buf(),
        });
    }

    Ok(revents & libc::POLLIN != 0)
}

fn push_events(events: impl Iterator<Item = InputEvent>, pending: &mut VecDeque<KeyEvent>) {
    for event in events {
        if event.event_type() != EventType::KEY {
            continue;
        }
        let kind = match event.value() {
            1 => KeyEventKind::Press,
            0 => KeyEventKind::Release,
            _ => continue,
        };
        pending.push_back(KeyEvent {
            code: event.code(),
            kind,
        });
    }
}

fn is_device_gone(source: &io::Error) -> bool {
    matches!(source.raw_os_error(), Some(libc::ENODEV) | Some(libc::EIO))
}

fn is_interrupted(source: &io::Error) -> bool {
    source.raw_os_error() == Some(libc::EINTR)
}

#[cfg(test)]
mod tests {
    use super::{KeyEvent, KeyEventKind, is_device_gone, is_interrupted, push_events};
    use evdev::InputEvent;
    use std::collections::VecDeque;
    use std::io;

    #[test]
    fn queues_press_and_release_events_dropping_repeats_and_other_types() {
        let events = vec![
            InputEvent::new(0x01, 30, 1),
            InputEvent::new(0x01, 30, 2),
            InputEvent::new(0x01, 30, 0),
            InputEvent::new(0x04, 4, 4),
            InputEvent::new(0x01, 103, 1),
        ];

        let mut pending = VecDeque::new();
        push_events(events.into_iter(), &mut pending);

        assert_eq!(
            pending.into_iter().collect::<Vec<_>>(),
            vec![
                KeyEvent {
                    code: 30,
                    kind: KeyEventKind::Press
                },
                KeyEvent {
                    code: 30,
                    kind: KeyEventKind::Release
                },
                KeyEvent {
                    code: 103,
                    kind: KeyEventKind::Press
                },
            ]
        );
    }

    #[test]
    fn detects_device_removal_errnos() {
        assert!(is_device_gone(&io::Error::from_raw_os_error(libc::ENODEV)));
        assert!(is_device_gone(&io::Error::from_raw_os_error(libc::EIO)));
        assert!(!is_device_gone(&io::Error::from_raw_os_error(libc::EAGAIN)));
        assert!(!is_device_gone(&io::Error::other("unrelated")));
    }

    #[test]
    fn detects_interrupted_errno() {
        assert!(is_interrupted(&io::Error::from_raw_os_error(libc::EINTR)));
        assert!(!is_interrupted(&io::Error::from_raw_os_error(libc::EAGAIN)));
        assert!(!is_interrupted(&io::Error::from_raw_os_error(libc::ENODEV)));
    }
}
