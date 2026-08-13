use evdev::Device;
use std::io;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use udu::device::discover_keyboards;

fn main() -> io::Result<()> {
    let device_name = std::env::args().nth(1);
    let path = resolve_device_path(device_name)?;

    let mut device = Device::open(&path)?;
    device.set_nonblocking(true)?;

    eprintln!(
        "Listening on {} — press End, PageDown, and RightShift (then Ctrl-C)",
        path.display()
    );
    eprintln!("Ctrl-C to exit.");

    let mut fds = [libc::pollfd {
        fd: device.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];

    loop {
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, 1000) };
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        if ready == 0 {
            continue;
        }

        if let Ok(events) = device.fetch_events() {
            for event in events {
                if event.event_type() == evdev::EventType::KEY && event.value() == 1 {
                    let code = event.code();
                    println!("press code={} key={:?}", code, evdev::KeyCode::new(code));
                }
            }
        }
    }
}

fn resolve_device_path(device_name: Option<String>) -> io::Result<PathBuf> {
    let keyboards = discover_keyboards().map_err(io::Error::other)?;

    if let Some(device_name) = device_name {
        if let Some(keyboard) = keyboards
            .iter()
            .find(|keyboard| keyboard.name == device_name)
        {
            return Ok(keyboard.path.clone());
        }

        return Err(io::Error::other(format!(
            "no keyboard named '{device_name}' was found under /dev/input"
        )));
    }

    if let Some(keyboard) = keyboards.first() {
        return Ok(keyboard.path.clone());
    }

    Err(io::Error::other(
        "no keyboard devices found under /dev/input (check permissions)",
    ))
}
