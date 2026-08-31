use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

pub const EXAMPLE: &str = "# GUI2TUI v0.1: all settings are optional. CLI overrides this file.\nversion = 1\n\n[runtime]\nbackend_timeout_ms = 5000\nevent_queue_capacity = 2048\n\n[terminal]\nmouse = true\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub runtime: RuntimeConfig,
    pub terminal: TerminalConfig,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub backend_timeout_ms: u64,
    pub event_queue_capacity: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalConfig {
    pub mouse: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            runtime: RuntimeConfig::default(),
            terminal: TerminalConfig::default(),
        }
    }
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            backend_timeout_ms: 5000,
            event_queue_capacity: 2048,
        }
    }
}
impl Default for TerminalConfig {
    fn default() -> Self {
        Self { mouse: true }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let result: Self = toml::from_str(text).map_err(|error: toml::de::Error| {
            let offset = error.span().map(|s| s.start).unwrap_or(0).min(text.len());
            let line = text.as_bytes()[..offset].iter().filter(|b| **b == b'\n').count() + 1;
            // Parser errors can contain a submitted value: never echo them.
            format!("Invalid TOML, unknown field, or wrong type at line {line}. See docs/configuration.md; run gui2tui config check")
        })?;
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported configuration version; expected version = 1".into());
        }
        if !(50..=30_000).contains(&self.runtime.backend_timeout_ms) {
            return Err("runtime.backend_timeout_ms must be 50..=30000".into());
        }
        if !(4..=65_536).contains(&self.runtime.event_queue_capacity) {
            return Err("runtime.event_queue_capacity must be 4..=65536".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err(format!("Cannot read {}; check permissions", path.display())),
        };
        let mut text = String::new();
        file.take(65537)
            .read_to_string(&mut text)
            .map_err(|_| "Configuration must be readable UTF-8".to_owned())?;
        if text.len() > 65536 {
            return Err("Configuration exceeds 64 KiB limit".into());
        }
        Self::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }
    pub fn apply_overrides(
        &mut self,
        timeout: Option<u64>,
        capacity: Option<usize>,
        no_mouse: bool,
    ) -> Result<(), String> {
        if let Some(timeout) = timeout {
            self.runtime.backend_timeout_ms = timeout;
        }
        if let Some(capacity) = capacity {
            self.runtime.event_queue_capacity = capacity;
        }
        if no_mouse {
            self.terminal.mouse = false;
        }
        self.validate()
    }
    pub fn init(path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(EXAMPLE.as_bytes())?;
        file.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_partial_config() {
        assert_eq!(
            Config::parse("").unwrap().runtime.event_queue_capacity,
            2048
        );
        assert!(
            !Config::parse("[terminal]\nmouse=false")
                .unwrap()
                .terminal
                .mouse
        );
        let temp = tempfile::tempdir().unwrap();
        assert!(Config::load(&temp.path().join("absent")).is_ok());
    }
    #[test]
    fn invalid_config_is_rejected_without_echoing_values() {
        for text in [
            "version=9",
            "[terminal]\nmouse='password-sentinel'",
            "unknown='password-sentinel'",
            "[runtime]\nbackend_timeout_ms=0",
            "[runtime]\nevent_queue_capacity=3",
            "not toml password-sentinel",
        ] {
            let error = Config::parse(text).unwrap_err();
            assert!(!error.contains("password-sentinel"));
        }
    }
    #[test]
    fn cli_precedence_and_no_silent_clamp() {
        let mut config = Config::parse("[runtime]\nbackend_timeout_ms=1000").unwrap();
        config.apply_overrides(Some(2000), None, true).unwrap();
        assert_eq!(config.runtime.backend_timeout_ms, 2000);
        assert_eq!(config.runtime.event_queue_capacity, 2048);
        assert!(!config.terminal.mouse);
        assert!(config.apply_overrides(None, Some(0), false).is_err());
    }
    #[test]
    fn init_never_overwrites_and_readonly_config_loads() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        Config::init(&path).unwrap();
        assert!(Config::init(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(Config::load(&path).is_ok());
    }
}
