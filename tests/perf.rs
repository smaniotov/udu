use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, InputEvent, KeyCode};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use udu::backend::audio::Audio;
use udu::backend::capture::Capture;
use udu::backend::mapping::Mapping;

const PRESS_CODES: [u16; 4] = [30, 31, 32, 33];
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
const DISPATCH_BUDGET_US: u128 = 1_000;
const LOOKUP_BUDGET_NS: u128 = 1_000_000;

fn bench_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wayvibes-perf-{name}-{}", std::process::id()))
}

fn write_sine_wav(path: &Path, samples: usize, sample_rate: u32) {
    let mut data = Vec::with_capacity(samples * 2);
    for index in 0..samples {
        let phase = 2.0 * std::f32::consts::PI * 440.0 * index as f32 / sample_rate as f32;
        let value = (phase.sin() * 0.25 * 32767.0) as i16;
        data.extend_from_slice(&value.to_le_bytes());
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    fs::write(path, bytes).expect("write wav");
}

fn write_bench_pack(root: &Path) -> PathBuf {
    fs::create_dir_all(root).expect("create pack dir");
    for index in 0..PRESS_CODES.len() {
        write_sine_wav(&root.join(format!("tone-{index}.wav")), 44, 44_100);
    }
    fs::write(
        root.join("config.json"),
        r#"{"name":"Perf Pack","key_define_type":"multi","defines":{"30":"tone-0.wav","31":"tone-1.wav","32":"tone-2.wav","33":"tone-3.wav"}}"#,
    )
    .expect("write config");
    root.to_path_buf()
}

fn emit_click(device: &mut VirtualDevice, code: u16) {
    device
        .emit(&[InputEvent::new(1, code, 1), InputEvent::new(1, code, 0)])
        .expect("emit click");
}

fn open_capture_with_retry(name: &str) -> Capture {
    for _attempt in 0..60 {
        if let Ok(capture) = Capture::open(name) {
            return capture;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    panic!("capture did not find the virtual device '{name}'");
}

fn percentile(latencies: &mut [u128], percentile: usize) -> u128 {
    latencies.sort_unstable();
    let index = ((latencies.len() - 1) * percentile / 100).min(latencies.len() - 1);
    latencies[index]
}

#[test]
#[ignore = "performance harness; run explicitly with --ignored"]
fn mapping_lookup_latency() {
    let _serial = SERIAL.lock().unwrap();
    let pack = write_bench_pack(&bench_directory("mapping"));
    let mapping = Mapping::load(&pack).expect("load mapping");

    let mut latencies = Vec::with_capacity(200_000);
    for index in 0..200_000u32 {
        let code = (index % 200) as u16;
        let start = Instant::now();
        let _ = mapping.lookup_down(code);
        latencies.push(start.elapsed().as_nanos());
    }

    let p95 = percentile(&mut latencies, 95);
    let mean = latencies.iter().copied().sum::<u128>() / latencies.len() as u128;
    println!("mapping lookup: mean={mean} ns p95={p95} ns over 200k lookups");

    assert!(
        p95 < LOOKUP_BUDGET_NS,
        "lookup p95 must stay well under 1 ms"
    );
    fs::remove_dir_all(&pack).expect("remove bench directory");
}

#[test]
#[ignore = "performance harness; run explicitly with --ignored"]
fn decode_cold_and_hot_playback() {
    let _serial = SERIAL.lock().unwrap();
    let Ok(audio) = Audio::new(1.0) else {
        eprintln!("no audio device available; skipping decode benchmark");
        return;
    };
    let pack = write_bench_pack(&bench_directory("decode"));
    let path = pack.join("tone-0.wav");

    let cold_start = Instant::now();
    for _ in 0..10 {
        audio.play(&path).expect("cold play");
    }
    let cold = cold_start.elapsed() / 10;

    let mut hot_latencies = Vec::with_capacity(200);
    for _ in 0..200 {
        let start = Instant::now();
        audio.play(&path).expect("hot play");
        hot_latencies.push(start.elapsed().as_nanos());
    }
    let hot_p95 = percentile(&mut hot_latencies, 95);
    let hot_mean = hot_latencies.iter().copied().sum::<u128>() / hot_latencies.len() as u128;
    let speedup = cold.as_nanos() as f64 / hot_mean.max(1) as f64;
    println!(
        "decode: cold={cold:?} (first decode) hot mean={hot_mean} ns p95={hot_p95} ns speedup={speedup:.0}x"
    );

    assert!(
        hot_p95 < LOOKUP_BUDGET_NS,
        "hot play must stay well under 1 ms"
    );
    fs::remove_dir_all(&pack).expect("remove bench directory");
}

#[test]
#[ignore = "performance harness; run explicitly with --ignored"]
fn uinput_dispatch_latency_and_drop_rate() {
    let _serial = SERIAL.lock().unwrap();
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in PRESS_CODES {
        keys.insert(KeyCode::new(code));
    }
    let device_name = format!("udu perf keyboard-{}", std::process::id());
    let mut device = VirtualDevice::builder()
        .expect("open uinput")
        .name(&device_name)
        .with_keys(&keys)
        .expect("register keys")
        .build()
        .expect("build virtual device");

    let mut capture = open_capture_with_retry(&device_name);

    let mut latencies = Vec::with_capacity(100);
    for index in 0..100 {
        let code = PRESS_CODES[index % PRESS_CODES.len()];
        let start = Instant::now();
        emit_click(&mut device, code);
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let mut spins = 0u32;
        let mut last: Vec<String> = Vec::new();
        loop {
            if Instant::now() >= deadline {
                panic!(
                    "press {index} (code {code}) did not arrive within 2 s; spins={spins} last={last:?}"
                );
            }
            match capture.next_key_event().expect("read event") {
                Some(event)
                    if event.code == code
                        && event.kind == udu::backend::capture::KeyEventKind::Press =>
                {
                    last.push(format!("hit{}", event.code));
                    break;
                }
                Some(event) => {
                    last.push(format!("other{}", event.code));
                }
                None => {
                    last.push(String::from("none"));
                }
            }
            spins += 1;
            if last.len() > 6 {
                last.remove(0);
            }
        }
        latencies.push(start.elapsed().as_micros());
    }
    let p95 = percentile(&mut latencies, 95);
    let mean = latencies.iter().copied().sum::<u128>() / latencies.len() as u128;
    println!("uinput dispatch: mean={mean} us p95={p95} us over 100 presses");

    assert!(p95 <= DISPATCH_BUDGET_US, "dispatch p95 must be <= 1 ms");

    for (rate_hz, press_count, label) in [
        (30u32, 60u32, "30/s sustained"),
        (100u32, 120u32, "100/s burst"),
    ] {
        let period = std::time::Duration::from_micros((1_000_000 / rate_hz.max(1)) as u64);
        let mut written = 0;
        let mut received = 0;
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let start = Instant::now();
        while written < press_count && Instant::now() < deadline {
            let code = PRESS_CODES[(written as usize) % PRESS_CODES.len()];
            emit_click(&mut device, code);
            written += 1;
            std::thread::sleep(period);
            while let Ok(Some(event)) = capture.next_key_event()
                && event.kind == udu::backend::capture::KeyEventKind::Press
            {
                received += 1;
            }
        }
        while received < written && Instant::now() < deadline {
            if let Ok(Some(event)) = capture.next_key_event()
                && event.kind == udu::backend::capture::KeyEventKind::Press
            {
                received += 1;
            }
        }
        let elapsed = start.elapsed();
        println!("drop rate {label}: written={written} received={received} elapsed={elapsed:?}");
        assert_eq!(received, written, "no keypresses may be dropped at {label}");
    }

    drop(capture);
    drop(device);
}

#[test]
#[ignore = "performance harness; run explicitly with --ignored"]
fn idle_tick_stays_poll_driven() {
    let _serial = SERIAL.lock().unwrap();
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::new(30));
    let device_name = format!("udu perf idle-{}", std::process::id());
    let device = VirtualDevice::builder()
        .expect("open uinput")
        .name(&device_name)
        .with_keys(&keys)
        .expect("register keys")
        .build()
        .expect("build virtual device");

    let mut capture = open_capture_with_retry(&device_name);

    let start = Instant::now();
    for _ in 0..20 {
        assert!(capture.next_key_event().expect("idle read").is_none());
    }
    let elapsed = start.elapsed();
    let mean_idle = elapsed / 20;
    println!("idle tick: mean={mean_idle:?} over 20 empty polls");

    assert!(
        mean_idle >= std::time::Duration::from_millis(1) - std::time::Duration::from_micros(200),
        "idle read must block on poll, not spin"
    );
    drop(capture);
    drop(device);
}
