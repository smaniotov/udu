use std::path::PathBuf;

use anyhow::{Context, Result};
use std::time::Duration;

use ratatui::crossterm::event;

use clap::Parser;
use udu::app::App;
use udu::cli::CliOptions;
use udu::config::{
    AppConfig, load_config, migrate_legacy_dirs, migrate_volume_scale, prepare_config,
};
use udu::service::run_service;
use udu::ui;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let options = CliOptions::parse();

    migrate_legacy_dirs().context("could not migrate legacy data directories")?;

    let config_path = options
        .config_path
        .clone()
        .or_else(default_config_path)
        .context("could not determine a user configuration directory")?;

    migrate_volume_scale(&config_path).context("could not migrate the volume scale")?;

    if options.service_mode {
        return run_service(&config_path).map_err(Into::into);
    }

    let mut config = load_config(&config_path)?;
    apply_cli_options(&mut config, options);
    let config = prepare_config(config)?;
    let mut app = App::new(config_path.clone(), config)?;
    app.start_backend()?;

    ratatui::run(|terminal| run_tui(terminal, &mut app))?;

    Ok(())
}

fn run_tui(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        app.poll_process()?;

        if event::poll(Duration::from_millis(100))? {
            ui::handle_event(app, event::read()?)?;
        }
    }

    Ok(())
}

fn default_config_path() -> Option<PathBuf> {
    udu::config::default_config_path()
}

fn apply_cli_options(config: &mut AppConfig, options: CliOptions) {
    if !options.soundpack_roots.is_empty() {
        config.soundpack_roots = options.soundpack_roots;
    }

    if options.selected_soundpack.is_some() {
        config.selected_soundpack = options.selected_soundpack;
    }

    if options.device_name.is_some() {
        config.device_name = options.device_name;
    }
}
