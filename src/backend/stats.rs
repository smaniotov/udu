use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SAVE_INTERVAL: Duration = Duration::from_secs(5);
const OWNER_ONLY: u32 = 0o600;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stats {
    pub keystrokes: u64,
    pub dings: u64,
    pub since: String,
    pub per_switch: BTreeMap<String, u64>,
}

impl Stats {
    pub fn started_now() -> Self {
        Self {
            keystrokes: 0,
            dings: 0,
            since: timestamp(),
            per_switch: BTreeMap::new(),
        }
    }
}

pub struct StatsStore {
    stats: Stats,
    path: PathBuf,
    last_save: Instant,
}

impl StatsStore {
    pub fn load_or_default(path: PathBuf) -> Self {
        let stats = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_else(Self::fresh);

        Self {
            stats,
            path,
            last_save: Instant::now(),
        }
    }

    fn fresh() -> Stats {
        Stats::started_now()
    }

    pub fn bump_keystroke(&mut self, pack: Option<&str>) {
        self.stats.keystrokes += 1;
        if let Some(pack) = pack {
            let entry = self.stats.per_switch.entry(pack.to_string()).or_insert(0);
            *entry += 1;
        }
        self.persist_if_due();
    }

    pub fn bump_ding(&mut self) {
        self.stats.dings += 1;
        self.persist_if_due();
    }

    fn persist_if_due(&mut self) {
        if self.last_save.elapsed() < SAVE_INTERVAL {
            return;
        }
        self.flush();
    }

    pub fn flush(&mut self) {
        if let Ok(serialized) = serde_json::to_string(&self.stats) {
            let _ = fs::create_dir_all(self.path.parent().unwrap_or(Path::new(".")));

            if let Err(error) = write_owner_only(&self.path, &serialized) {
                eprintln!("could not save usage stats: {error}");
            }
        }
        self.last_save = Instant::now();
    }

    pub fn snapshot(&self) -> Stats {
        self.stats.clone()
    }

    pub fn reset(&mut self) {
        self.stats = Self::fresh();
        self.flush();
    }

    pub fn export_markdown(&self) -> String {
        let mut lines = vec!["# udu usage stats".to_string(), String::new()];
        lines.push(format!("Since: {}", self.stats.since));
        lines.push(format!("Keystrokes: {}", self.stats.keystrokes));
        lines.push(format!("Return dings: {}", self.stats.dings));
        lines.push(String::new());
        lines.push("Per switch:".to_string());
        for (switch, count) in &self.stats.per_switch {
            lines.push(format!("- {switch}: {count}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

fn write_owner_only(path: &Path, contents: &str) -> std::io::Result<()> {
    fs::write(path, contents)?;

    let mut permissions = fs::metadata(path)?.permissions();

    if permissions.mode() & 0o777 == OWNER_ONLY {
        return Ok(());
    }

    permissions.set_mode(OWNER_ONLY);

    fs::set_permissions(path, permissions)
}

fn timestamp() -> String {
    format!("{:?}", std::time::SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::{OWNER_ONLY, Stats, StatsStore};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    fn flushing_tightens_a_world_readable_stats_file_to_owner_only() {
        let dir = std::env::temp_dir().join(format!("udu-stats-mode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("stats.json");
        std::fs::write(&path, "{}").expect("seed a permissive file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("make it world readable");

        let mut store = StatsStore::load_or_default(path.clone());
        store.bump_keystroke(Some("Creams"));
        store.flush();

        let mode = std::fs::metadata(&path)
            .expect("read metadata")
            .permissions()
            .mode();

        assert_eq!(
            mode & 0o777,
            OWNER_ONLY,
            "a stats file that predates the fix must be tightened, since fs::write keeps the old mode"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stats_persist_and_reload() {
        let dir = std::env::temp_dir().join(format!("udu-stats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("stats.json");
        let mut store = StatsStore::load_or_default(path.clone());
        store.stats.since = String::from("t");
        store.bump_keystroke(Some("Creams"));
        store.bump_keystroke(Some("Creams"));
        store.bump_ding();
        store.flush();

        let reloaded = StatsStore::load_or_default(path.clone());
        assert_eq!(reloaded.snapshot().keystrokes, 2);
        assert_eq!(reloaded.snapshot().dings, 1);
        assert_eq!(reloaded.snapshot().per_switch.get("Creams"), Some(&2));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn export_is_markdown_with_counts() {
        let mut stats = Stats::started_now();
        stats.keystrokes = 3;
        stats.dings = 1;
        stats.per_switch.insert(String::from("Oreo"), 2);
        let store = StatsStore {
            stats,
            path: PathBuf::new(),
            last_save: std::time::Instant::now(),
        };

        let md = store.export_markdown();
        assert!(md.contains("Keystrokes: 3"));
        assert!(md.contains("Return dings: 1"));
        assert!(md.contains("Oreo: 2"));
        assert!(md.starts_with("# udu usage stats"));
    }
}
