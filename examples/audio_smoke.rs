use std::path::PathBuf;
use std::time::Duration;

use udu::backend::audio::Audio;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let audio = Audio::new(1.0)?;
    eprintln!("audio stream started, master volume {}", audio.volume());

    let fixture = |name: &str| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    };

    for name in ["tone.wav", "tone.mp3"] {
        audio.play(&fixture(name))?;
        eprintln!("played {name}");
        std::thread::sleep(Duration::from_millis(200));
    }

    audio.set_master_volume(0.5);
    eprintln!("volume -> {}", audio.volume());
    audio.play(&fixture("tone.wav"))?;
    std::thread::sleep(Duration::from_millis(300));

    eprintln!("stream_failed = {}", audio.stream_failed());
    Ok(())
}
