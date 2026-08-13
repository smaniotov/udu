use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::prelude::Color;
use ratatui::prelude::Modifier;
use udu::app::{App, Screen, SettingsTab};
use udu::config::AppConfig;
use udu::device::KeyboardDevice;
use udu::soundpack::Soundpack;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 100)]
    cols: u16,
    #[arg(long, default_value_t = 30)]
    rows: u16,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value = "soundpacks")]
    state: String,
    #[arg(long)]
    live: bool,
    #[arg(long)]
    size_file: Option<PathBuf>,
}

fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => (255, 255, 255),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::Gray => (150, 150, 150),
        Color::DarkGray => (80, 80, 80),
        Color::LightRed => (252, 146, 158),
        Color::LightGreen => (130, 255, 165),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (114, 159, 207),
        Color::LightMagenta => (173, 127, 168),
        Color::LightCyan => (92, 220, 229),
        Color::White => (242, 242, 242),
        _ => (255, 255, 255),
    }
}

fn sample_packs() -> Vec<Soundpack> {
    [
        ("nk-cream", 128),
        ("membrane-60", 96),
        ("topre-30g", 241),
        ("mx-brown", 104),
        ("silent-red", 87),
    ]
    .iter()
    .map(|(name, count)| Soundpack {
        name: name.to_string(),
        path: PathBuf::from(format!("/packs/{name}")),
        mapping_count: *count,
    })
    .collect()
}

fn sample_devices() -> Vec<KeyboardDevice> {
    [
        ("Logitech ERGO K860", "/dev/input/event5"),
        ("Keychron K8 Pro", "/dev/input/event9"),
        ("Wooting 60HE", "/dev/input/event12"),
    ]
    .iter()
    .map(|(name, path)| KeyboardDevice {
        name: name.to_string(),
        path: PathBuf::from(*path),
    })
    .collect()
}

fn seeded_app() -> anyhow::Result<App> {
    let mut app = App::new(
        PathBuf::from("/tmp/udu-snapshot.json"),
        AppConfig::default(),
    )?;
    app.packs = sample_packs();
    app.devices = sample_devices();
    app.config.volume = 7.5;
    app.config.soundpack_roots = dirs::home_dir()
        .map(|home| vec![home.join(".local/share/udu/soundpacks")])
        .unwrap_or_default();
    app.status = "Soundpack: nk-cream — playing".to_string();
    app.list_state.select(Some(0));
    app.device_list_state.select(Some(0));
    Ok(app)
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut app = match seeded_app() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("could not create app: {e}");
            return ExitCode::FAILURE;
        }
    };

    match args.state.as_str() {
        "general" => {
            app.open_settings();
            app.settings_tab = SettingsTab::General;
        }
        "devices" => {
            app.open_settings();
            app.settings_tab = SettingsTab::Devices;
        }
        "audio" => {
            app.open_settings();
            app.settings_tab = SettingsTab::Audio;
        }
        "about" => {
            app.open_settings();
            app.settings_tab = SettingsTab::About;
        }
        _ => app.screen = Screen::Launcher,
    }

    if args.live {
        return run_live(app, args.size_file.clone());
    }

    let backend = TestBackend::new(args.cols, args.rows);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("could not create test terminal: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = terminal.draw(|frame| udu::ui::draw(frame, &mut app)) {
        eprintln!("draw failed: {e}");
        return ExitCode::FAILURE;
    }

    let out = args
        .out
        .unwrap_or_else(|| PathBuf::from("/tmp/udu-snapshot.html"));
    if let Err(e) = std::fs::write(&out, to_html(terminal.backend().buffer())) {
        eprintln!("could not write {out:?}: {e}");
        return ExitCode::FAILURE;
    }
    println!("{}", out.display());
    ExitCode::SUCCESS
}

fn run_live(mut app: App, size_file: Option<PathBuf>) -> ExitCode {
    match ratatui::run(|terminal| run_tui(terminal, &mut app, size_file)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tui error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_tui(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    size_file: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use ratatui::crossterm::event;

    let size = terminal.size()?;
    if let Some(path) = size_file {
        let _ = std::fs::write(path, format!("{}x{}", size.width, size.height));
    }

    while !app.should_quit {
        terminal.draw(|frame| udu::ui::draw(frame, app))?;
        app.poll_process()?;

        if event::poll(Duration::from_millis(100))? {
            udu::ui::handle_event(app, event::read()?)?;
        }
    }

    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn to_html(buf: &Buffer) -> String {
    let (cols, rows) = (buf.area.width, buf.area.height);
    let mut body = String::new();
    body.push_str(r#"<div class="term"><pre>"#);

    for y in 0..rows {
        for x in 0..cols {
            let cell = buf.cell((x, y)).expect("in-bounds cell");
            let (fr, fg, fb) = rgb(cell.fg);
            let (br, bg, bb) = rgb(cell.bg);
            let mut style_class = String::new();
            if cell.modifier.contains(Modifier::BOLD) {
                style_class.push_str(" font-bold");
            }
            if cell.modifier.contains(Modifier::DIM) {
                style_class.push_str(" dim");
            }
            if cell.modifier.contains(Modifier::REVERSED) {
                style_class.push_str(" reversed");
            }
            body.push_str(&format!(
                r#"<span style="color:rgb({fr},{fg},{fb});background:rgb({br},{bg},{bb})" class="{style_class}">{}</span>"#,
                esc(cell.symbol())
            ));
        }
        body.push('\n');
    }
    body.push_str("</pre></div>");

    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;padding:0;background:#111}}
.term{{padding:8px}}
pre{{font-family:'JetBrains Mono','Fira Code','SF Mono',Menlo,Consolas,monospace;font-size:16px;line-height:19px;margin:0;letter-spacing:0;white-space:pre}}
span{{display:inline}}
.font-bold{{font-weight:700}}
.dim{{opacity:.55}}
.reversed{{filter:invert(1)}}
</style></head><body>{body}</body></html>"#
    )
}
