use crate::config::{AppConfig, ConfigError, clamp_volume, migrate_volume_scale, save_config};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub const SERVICE_NAME: &str = "udu.service";
const LEGACY_SERVICE_NAMES: [&str; 1] = ["wayvibes-tui.service"];

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("the udu backend failed: {0}")]
    Backend(#[from] crate::backend::BackendError),
    #[error("could not resolve the udu executable path")]
    ResolveExecutable,
    #[error(
        "the udu executable at {} was deleted; reinstall or restart udu before enabling the service",
        path.display()
    )]
    ExecutableDeleted { path: PathBuf },
    #[error("could not resolve the systemd user configuration directory")]
    ResolveConfigDirectory,
    #[error("could not write systemd unit {}: {source}", path.display())]
    WriteUnit {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the path {} cannot appear in a systemd ExecStart line", path.display())]
    InvalidUnitPath { path: PathBuf },
    #[error("systemd action '{action}' failed: {details}")]
    Systemctl { action: String, details: String },
    #[error("could not access the udu runtime state: {details}")]
    Runtime { details: String },
    #[error("another udu backend is already running (lock: {})", path.display())]
    AlreadyRunning { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyUnitOutcome {
    Migrated,
    MigratedDisableFailed { details: String },
    SkippedNotOwnedByUdu,
    SkippedUnreadable { details: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyUnitMigration {
    pub unit_name: &'static str,
    pub outcome: LegacyUnitOutcome,
}

#[derive(Debug, Default)]
pub struct UduService;

impl UduService {
    pub fn start_service(
        &self,
        config_path: &Path,
        config: &AppConfig,
    ) -> Result<Vec<LegacyUnitMigration>, ServiceError> {
        save_config(config_path, &normalized_config(config))?;
        let executable = resolve_executable()?;
        let migrations = migrate_legacy_unit(&executable)?;
        self.install_unit(config_path, &executable)?;
        self.systemctl("enable")?;
        self.systemctl("start")?;

        Ok(migrations)
    }

    pub fn stop_and_uninstall(&self) -> Result<(), ServiceError> {
        let unit_path = self.unit_path()?;

        uninstall_unit_at(&unit_path)
    }

    pub fn is_installed(&self) -> bool {
        self.unit_path().map(|path| path.exists()).unwrap_or(false)
    }

    fn unit_path(&self) -> Result<PathBuf, ServiceError> {
        Ok(unit_directory()?.join(SERVICE_NAME))
    }

    fn install_unit(&self, config_path: &Path, executable: &Path) -> Result<(), ServiceError> {
        let unit_directory = unit_directory()?;
        let unit_path = unit_directory.join(SERVICE_NAME);
        let unit = render_service_unit(executable, config_path)?;

        fs::create_dir_all(&unit_directory).map_err(|source| ServiceError::WriteUnit {
            path: unit_directory.clone(),
            source,
        })?;
        fs::write(&unit_path, unit).map_err(|source| ServiceError::WriteUnit {
            path: unit_path,
            source,
        })?;
        self.systemctl("daemon-reload")
    }

    fn systemctl(&self, action: &str) -> Result<(), ServiceError> {
        run_systemctl(&systemctl_args(action), action)
    }
}

pub fn run_service(config_path: &Path) -> Result<(), ServiceError> {
    let _lock = ServiceLock::acquire()?;
    migrate_volume_scale(config_path)?;
    let config = normalized_config(&crate::config::load_config(config_path)?);

    crate::backend::run(config_path, config)?;

    Ok(())
}

fn resolve_executable() -> Result<PathBuf, ServiceError> {
    let executable = std::env::current_exe().map_err(|_| ServiceError::ResolveExecutable)?;

    if is_deleted_executable_path(&executable) {
        return Err(ServiceError::ExecutableDeleted { path: executable });
    }

    executable
        .canonicalize()
        .map_err(|_| ServiceError::ResolveExecutable)
}

fn is_deleted_executable_path(path: &Path) -> bool {
    path.to_string_lossy().ends_with(" (deleted)")
}

fn unit_directory() -> Result<PathBuf, ServiceError> {
    dirs::config_dir()
        .map(|directory| directory.join("systemd/user"))
        .ok_or(ServiceError::ResolveConfigDirectory)
}

fn uninstall_unit_at(unit_path: &Path) -> Result<(), ServiceError> {
    if !unit_path.exists() {
        return Ok(());
    }

    run_systemctl(&["--user", "disable", "--now", SERVICE_NAME], "disable")?;
    fs::remove_file(unit_path).map_err(|source| ServiceError::WriteUnit {
        path: unit_path.to_path_buf(),
        source,
    })?;

    run_systemctl(&["--user", "daemon-reload"], "daemon-reload")
}

fn migrate_legacy_unit(executable: &Path) -> Result<Vec<LegacyUnitMigration>, ServiceError> {
    let directory = unit_directory()?;
    let mut migrations = Vec::new();

    for legacy in LEGACY_SERVICE_NAMES {
        let legacy_path = directory.join(legacy);

        if !legacy_path.exists() {
            continue;
        }

        let outcome = migrate_one_legacy_unit(&legacy_path, legacy, executable)?;
        migrations.push(LegacyUnitMigration {
            unit_name: legacy,
            outcome,
        });
    }

    run_systemctl(&["--user", "daemon-reload"], "daemon-reload")?;

    Ok(migrations)
}

fn migrate_one_legacy_unit(
    legacy_path: &Path,
    legacy: &'static str,
    executable: &Path,
) -> Result<LegacyUnitOutcome, ServiceError> {
    let contents = match fs::read_to_string(legacy_path) {
        Ok(contents) => contents,
        Err(error) => {
            return Ok(LegacyUnitOutcome::SkippedUnreadable {
                details: error.to_string(),
            });
        }
    };

    if !unit_targets_udu(&contents, executable) {
        return Ok(LegacyUnitOutcome::SkippedNotOwnedByUdu);
    }

    let disable_result = run_systemctl(
        &["--user", "disable", "--now", legacy],
        &format!("disable legacy unit {legacy}"),
    );
    let backup_path = legacy_path.with_extension("service.udu-backup");

    fs::rename(legacy_path, &backup_path).map_err(|source| ServiceError::WriteUnit {
        path: legacy_path.to_path_buf(),
        source,
    })?;

    match disable_result {
        Ok(()) => Ok(LegacyUnitOutcome::Migrated),
        Err(error) => Ok(LegacyUnitOutcome::MigratedDisableFailed {
            details: error.to_string(),
        }),
    }
}

fn unit_targets_udu(unit_contents: &str, executable: &Path) -> bool {
    let Some(exec_start) = unit_contents
        .lines()
        .find_map(|line| line.strip_prefix("ExecStart="))
    else {
        return false;
    };

    let Some(command_name) = exec_start
        .split_whitespace()
        .next()
        .map(Path::new)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return false;
    };

    let current_name = executable.file_name().and_then(|name| name.to_str());

    command_name == "udu" || Some(command_name) == current_name
}

struct ServiceLock {
    file: fs::File,
}

impl ServiceLock {
    fn acquire() -> Result<Self, ServiceError> {
        let runtime_directory = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| ServiceError::Runtime {
                details: String::from("XDG_RUNTIME_DIR is not available for the service lock"),
            })?;
        let path = runtime_directory.join("udu.service.lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| ServiceError::Runtime {
                details: format!("could not open lock {}: {error}", path.display()),
            })?;

        let result = unsafe {
            libc::flock(
                std::os::fd::AsRawFd::as_raw_fd(&file),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        };

        if result == 0 {
            return Ok(Self { file });
        }

        let error = std::io::Error::last_os_error();

        if is_lock_contended(&error) {
            return Err(ServiceError::AlreadyRunning { path });
        }

        Err(ServiceError::Runtime {
            details: format!("could not lock {}: {error}", path.display()),
        })
    }
}

impl Drop for ServiceLock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&self.file), libc::LOCK_UN) };
    }
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(errno) if errno == libc::EWOULDBLOCK || errno == libc::EAGAIN)
}

fn normalized_config(config: &AppConfig) -> AppConfig {
    AppConfig {
        volume: clamp_volume(config.volume),
        ..config.clone()
    }
}

fn render_service_unit(executable: &Path, config_path: &Path) -> Result<String, ServiceError> {
    let executable = quote_unit_path(executable)?;
    let config_path = quote_unit_path(config_path)?;

    Ok(format!(
        "[Unit]\n\
         Description=Persistent udu keyboard sound backend\n\
         StartLimitIntervalSec=60\n\
         StartLimitBurst=10\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={executable} --service --config {config_path}\n\
         Restart=on-failure\n\
         RestartSec=2s\n\
         NoNewPrivileges=yes\n\
         SystemCallArchitectures=native\n\
         SystemCallFilter=@system-service\n\
         RestrictAddressFamilies=AF_UNIX\n\
         RestrictSUIDSGID=yes\n\
         LockPersonality=yes\n\
         MemoryDenyWriteExecute=yes\n\
         ProtectKernelTunables=yes\n\
         ProtectKernelModules=yes\n\
         ProtectControlGroups=yes\n\
         PrivateTmp=yes\n\
         UMask=0077\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    ))
}

fn run_systemctl(args: &[&str], action: &str) -> Result<(), ServiceError> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .map_err(|error| ServiceError::Systemctl {
            action: action.to_string(),
            details: error.to_string(),
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(ServiceError::Systemctl {
        action: action.to_string(),
        details: command_output(&output.stdout, &output.stderr),
    })
}

fn systemctl_args(action: &str) -> Vec<&str> {
    if action == "daemon-reload" {
        return vec!["--user", "daemon-reload"];
    }

    vec!["--user", action, SERVICE_NAME]
}

fn command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = stdout.trim();
    let stderr = stderr.trim();

    if stdout.is_empty() && stderr.is_empty() {
        return String::from("no output");
    }

    format!("{stdout}\n{stderr}")
}

fn quote_unit_path(path: &Path) -> Result<String, ServiceError> {
    let raw = path.to_string_lossy();

    if raw.contains(['\n', '\r', '\0']) {
        return Err(ServiceError::InvalidUnitPath {
            path: path.to_path_buf(),
        });
    }

    let escaped = raw
        .chars()
        .fold(String::with_capacity(raw.len()), |mut acc, ch| {
            match ch {
                '\\' => acc.push_str("\\\\"),
                '"' => acc.push_str("\\\""),
                '%' => acc.push_str("%%"),
                other => acc.push(other),
            }
            acc
        });

    Ok(format!("\"{escaped}\""))
}

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_SERVICE_NAMES, LegacyUnitOutcome, SERVICE_NAME, ServiceError,
        is_deleted_executable_path, is_lock_contended, migrate_one_legacy_unit, normalized_config,
        quote_unit_path, render_service_unit, resolve_executable, uninstall_unit_at,
        unit_targets_udu,
    };
    use crate::config::AppConfig;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn normalizes_service_volume_to_the_udu_range() {
        let config = AppConfig {
            volume: 150.0,
            ..AppConfig::default()
        };

        assert_eq!(normalized_config(&config).volume, 100.0);
    }

    #[test]
    fn quotes_paths_with_spaces_as_a_systemd_double_quoted_string() {
        assert_eq!(
            quote_unit_path(&PathBuf::from("/tmp/my config.json")).unwrap(),
            "\"/tmp/my config.json\""
        );
    }

    #[test]
    fn doubles_percent_signs_to_avoid_systemd_specifier_expansion() {
        assert_eq!(
            quote_unit_path(&PathBuf::from("/tmp/50%off/config.json")).unwrap(),
            "\"/tmp/50%%off/config.json\""
        );
    }

    #[test]
    fn escapes_backslashes_and_double_quotes() {
        assert_eq!(
            quote_unit_path(&PathBuf::from("/tmp/say \"hi\"\\here")).unwrap(),
            "\"/tmp/say \\\"hi\\\"\\\\here\""
        );
    }

    #[test]
    fn rejects_a_path_containing_a_newline() {
        let error = quote_unit_path(&PathBuf::from("/tmp/evil\nMalicious=yes")).unwrap_err();

        assert!(matches!(error, ServiceError::InvalidUnitPath { .. }));
    }

    #[test]
    fn uses_udu_as_the_fixed_service_name() {
        assert_eq!(SERVICE_NAME, "udu.service");
        assert_eq!(LEGACY_SERVICE_NAMES, ["wayvibes-tui.service"]);
    }

    #[test]
    fn unit_template_runs_udu_in_service_mode() {
        let unit =
            render_service_unit(Path::new("/tmp/udu"), Path::new("/tmp/config.json")).unwrap();

        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("ExecStart=\"/tmp/udu\" --service --config \"/tmp/config.json\""));
        assert!(unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn unit_template_hardens_the_service_without_breaking_required_capabilities() {
        let unit =
            render_service_unit(Path::new("/tmp/udu"), Path::new("/tmp/config.json")).unwrap();

        for directive in [
            "NoNewPrivileges=yes",
            "SystemCallArchitectures=native",
            "SystemCallFilter=@system-service",
            "RestrictAddressFamilies=AF_UNIX",
            "RestrictSUIDSGID=yes",
            "LockPersonality=yes",
            "MemoryDenyWriteExecute=yes",
            "ProtectKernelTunables=yes",
            "ProtectKernelModules=yes",
            "ProtectControlGroups=yes",
            "PrivateTmp=yes",
            "UMask=0077",
            "StartLimitIntervalSec=60",
            "StartLimitBurst=10",
        ] {
            assert!(unit.contains(directive), "missing {directive}");
        }

        for forbidden in [
            "PrivateDevices",
            "ProtectHome",
            "PrivateUsers",
            "RestrictRealtime",
            "ProtectSystem",
        ] {
            assert!(!unit.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn recognizes_an_exec_start_line_as_udus_own() {
        assert!(unit_targets_udu(
            "[Service]\nExecStart=/home/user/.local/bin/udu --service --config /tmp/config.json\n",
            Path::new("/home/user/.local/bin/udu"),
        ));
        assert!(unit_targets_udu(
            "[Service]\nExecStart=/opt/other/udu --service --config /tmp/config.json\n",
            Path::new("/somewhere/else/renamed-binary"),
        ));
    }

    #[test]
    fn does_not_recognize_a_third_party_binary_as_udus_own() {
        assert!(!unit_targets_udu(
            "[Service]\nExecStart=/usr/bin/other-keysound-app --daemon\n",
            Path::new("/home/user/.local/bin/udu"),
        ));
        assert!(!unit_targets_udu(
            "[Unit]\nDescription=no ExecStart here\n",
            Path::new("/home/user/.local/bin/udu"),
        ));
    }

    #[test]
    fn skips_a_legacy_unit_not_owned_by_udu_and_leaves_it_untouched() {
        let legacy_path = std::env::temp_dir().join(format!(
            "udu-legacy-foreign-{}-{}.service",
            std::process::id(),
            line!()
        ));
        fs::write(
            &legacy_path,
            "[Service]\nExecStart=/usr/bin/other-keysound-app --daemon\n",
        )
        .expect("write a foreign unit file");

        let outcome = migrate_one_legacy_unit(
            &legacy_path,
            "wayvibes-tui.service",
            Path::new("/usr/bin/udu"),
        )
        .expect("reading a foreign unit must not fail");

        assert_eq!(outcome, LegacyUnitOutcome::SkippedNotOwnedByUdu);
        assert!(legacy_path.exists(), "a foreign unit must be left in place");

        let _ = fs::remove_file(&legacy_path);
    }

    #[test]
    fn skips_an_unreadable_legacy_unit_without_acting_on_it() {
        let legacy_path = std::env::temp_dir().join(format!(
            "udu-legacy-unreadable-{}-{}.service",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(&legacy_path).expect("create a directory to make the read fail");

        let outcome = migrate_one_legacy_unit(
            &legacy_path,
            "wayvibes-tui.service",
            Path::new("/usr/bin/udu"),
        )
        .expect("an unreadable unit must not error out");

        assert!(matches!(
            outcome,
            LegacyUnitOutcome::SkippedUnreadable { .. }
        ));

        let _ = fs::remove_dir_all(&legacy_path);
    }

    #[test]
    fn uninstalling_a_missing_unit_is_a_no_op_success() {
        let unit_path = std::env::temp_dir().join(format!(
            "udu-missing-unit-{}-{}.service",
            std::process::id(),
            line!()
        ));

        assert!(!unit_path.exists());
        assert!(uninstall_unit_at(&unit_path).is_ok());
    }

    #[test]
    fn recognizes_a_deleted_executable_path() {
        assert!(is_deleted_executable_path(Path::new(
            "/home/user/.local/bin/udu (deleted)"
        )));
        assert!(!is_deleted_executable_path(Path::new(
            "/home/user/.local/bin/udu"
        )));
    }

    #[test]
    fn resolves_the_current_executable_to_a_canonical_existing_path() {
        let executable = resolve_executable().expect("the test binary itself must resolve");

        assert!(executable.is_absolute());
        assert!(executable.exists());
    }

    #[test]
    fn treats_ewouldblock_and_eagain_as_lock_contention() {
        let contended = std::io::Error::from_raw_os_error(libc::EWOULDBLOCK);
        let interrupted = std::io::Error::from_raw_os_error(libc::EINTR);

        assert!(is_lock_contended(&contended));
        assert!(!is_lock_contended(&interrupted));
    }

    #[test]
    fn wraps_a_backend_failure_with_an_actionable_message() {
        let error = ServiceError::from(crate::backend::BackendError::SocketPath(String::from(
            "no runtime directory",
        )));

        assert!(matches!(error, ServiceError::Backend(_)));
        assert!(error.to_string().contains("udu backend failed"));
    }
}
