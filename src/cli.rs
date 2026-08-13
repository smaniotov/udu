use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Default, Parser)]
#[command(
    name = "udu",
    version,
    about = "Mechanical keyboard sounds for your Linux desktop",
    after_help = "Keys:\n  Tab      Switch between soundpacks and devices\n  Up/Down  Select an item in the focused list\n  Enter    Activate the selected item\n  +/-      Change volume\n  r        Refresh soundpacks and devices\n  ?        Open the full help modal\n  q/Esc    Close the TUI; service keeps running"
)]
pub struct CliOptions {
    #[arg(
        long = "config",
        value_name = "PATH",
        help = "Use a specific manager config file"
    )]
    pub config_path: Option<PathBuf>,

    #[arg(
        long = "root",
        value_name = "PATH",
        help = "Add a soundpack discovery root"
    )]
    pub soundpack_roots: Vec<PathBuf>,

    #[arg(
        long = "soundpack",
        value_name = "PATH",
        help = "Select a soundpack explicitly"
    )]
    pub selected_soundpack: Option<PathBuf>,

    #[arg(
        long = "device-name",
        value_name = "NAME",
        help = "Pass an exact device name to the backend"
    )]
    pub device_name: Option<String>,

    #[arg(long = "service", help = "Run as the systemd-managed backend service")]
    pub service_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::CliOptions;
    use clap::CommandFactory;
    use clap::Parser;
    use clap::error::ErrorKind;
    use std::path::PathBuf;

    #[test]
    fn parses_runtime_overrides() {
        let options = CliOptions::try_parse_from([
            "udu",
            "--config",
            "/tmp/config.json",
            "--root",
            "/sounds",
            "--soundpack",
            "/sounds/quiet",
            "--device-name",
            "USB Keyboard",
        ])
        .expect("parse options");

        assert_eq!(options.config_path, Some(PathBuf::from("/tmp/config.json")));
        assert_eq!(options.soundpack_roots, [PathBuf::from("/sounds")]);
        assert_eq!(
            options.selected_soundpack,
            Some(PathBuf::from("/sounds/quiet"))
        );
        assert_eq!(options.device_name.as_deref(), Some("USB Keyboard"));
        assert!(!options.service_mode);
    }

    #[test]
    fn parses_repeatable_roots() {
        let options = CliOptions::try_parse_from(["udu", "--root", "/a", "--root", "/b"])
            .expect("parse roots");

        assert_eq!(
            options.soundpack_roots,
            [PathBuf::from("/a"), PathBuf::from("/b")]
        );
    }

    #[test]
    fn reports_missing_option_values() {
        let error =
            CliOptions::try_parse_from(["udu", "--root"]).expect_err("missing value should fail");

        assert!(error.to_string().contains("--root"));
    }

    #[test]
    fn reports_unknown_options() {
        let error =
            CliOptions::try_parse_from(["udu", "--bogus"]).expect_err("unknown option should fail");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn help_lists_every_option() {
        let help = CliOptions::command().render_help().to_string();

        for option in [
            "--config",
            "--root",
            "--soundpack",
            "--device-name",
            "--service",
        ] {
            assert!(help.contains(option), "help should mention {option}");
        }
    }
}
