use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

pub const EXAMPLE: &str = "# GUI2TUI settings are optional. CLI overrides this file.\nversion = 1\n\n[runtime]\nbackend_timeout_ms = 5000\nevent_queue_capacity = 2048\n\n[terminal]\nmouse = true\n\n# Optional shell-free handler for qualified complete multiline plain text.\n# The {file} argument is a GUI2TUI-owned private representation, never the app file.\n# [interaction.complex_text]\n# program = \"custom-editor-command\"\n# args = [\"--wait\", \"{file}\"]\n\n# Save launchers with `gui2tui app add`; do not hand-write shell commands.\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub runtime: RuntimeConfig,
    pub terminal: TerminalConfig,
    pub interaction: InteractionConfig,
    pub launchers: BTreeMap<String, LauncherConfig>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct InteractionConfig {
    pub complex_text: Option<TextInteractionHandlerConfig>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TextInteractionHandlerConfig {
    pub program: String,
    pub args: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct LauncherConfig {
    pub program: String,
    pub args: Vec<String>,
    pub match_name: String,
    pub wait_ms: u64,
    pub verified: bool,
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
            interaction: InteractionConfig::default(),
            launchers: BTreeMap::new(),
        }
    }
}
impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: Vec::new(),
            match_name: String::new(),
            wait_ms: 15_000,
            verified: false,
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
impl Default for TextInteractionHandlerConfig {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: vec!["{file}".to_owned()],
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
        if let Some(handler) = &self.interaction.complex_text {
            if handler.program.is_empty() || handler.program.len() > 4096 {
                return Err("interaction.complex_text.program must be 1..=4096 bytes".into());
            }
            if handler.args.len() > 128 || handler.args.iter().any(|arg| arg.len() > 4096) {
                return Err("interaction.complex_text.args exceeds the safe limit".into());
            }
            if handler
                .args
                .iter()
                .filter(|arg| arg.as_str() == "{file}")
                .count()
                != 1
                || handler
                    .args
                    .iter()
                    .any(|arg| arg.contains("{file}") && arg != "{file}")
            {
                return Err(
                    "interaction.complex_text.args must contain exactly one standalone {file} argument"
                        .into(),
                );
            }
            let executable = Path::new(&handler.program)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if matches!(executable, "sh" | "bash" | "dash" | "zsh" | "ksh" | "fish")
                && handler.args.iter().any(|arg| arg == "-c")
            {
                return Err("interaction.complex_text does not permit shell -c evaluation".into());
            }
        }
        for (id, launcher) in &self.launchers {
            if id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            {
                return Err(
                    "launcher id must use 1..=64 ASCII letters, digits, '.', '_' or '-'".into(),
                );
            }
            if launcher.program.is_empty() || launcher.program.len() > 4096 {
                return Err(format!("launchers.{id}.program must be 1..=4096 bytes"));
            }
            if launcher.match_name.is_empty() || launcher.match_name.len() > 256 {
                return Err(format!("launchers.{id}.match_name must be 1..=256 bytes"));
            }
            if launcher.args.len() > 128 || launcher.args.iter().any(|arg| arg.len() > 4096) {
                return Err(format!("launchers.{id}.args exceeds the safe limit"));
            }
            if !(100..=120_000).contains(&launcher.wait_ms) {
                return Err(format!("launchers.{id}.wait_ms must be 100..=120000"));
            }
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

    /// Atomically replace the user-owned configuration without following a
    /// pre-existing config-file symlink.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        if let Ok(metadata) = fs::symlink_metadata(path)
            && metadata.file_type().is_symlink()
        {
            return Err(format!("Refusing to replace symlink {}", path.display()));
        }
        let parent = path
            .parent()
            .ok_or_else(|| "Configuration path has no parent".to_owned())?;
        fs::create_dir_all(parent).map_err(|_| "Cannot create configuration directory")?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".config-")
            .tempfile_in(parent)
            .map_err(|_| "Cannot create temporary configuration file")?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "Cannot secure temporary configuration file")?;
        let encoded = toml::to_string_pretty(self)
            .map_err(|_| "Cannot serialize configuration".to_owned())?;
        temporary
            .write_all(encoded.as_bytes())
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| "Cannot write configuration")?;
        temporary
            .persist(path)
            .map_err(|_| format!("Cannot replace {}", path.display()))?;
        Ok(())
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
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        Config::init(&path).unwrap();
        assert!(Config::init(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(Config::load(&path).is_ok());
    }

    #[test]
    fn launcher_round_trip_and_validation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut config = Config::default();
        config.launchers.insert(
            "chromium".into(),
            LauncherConfig {
                program: "chromium".into(),
                args: vec!["--force-renderer-accessibility=complete".into()],
                match_name: "Google Chrome".into(),
                wait_ms: 20_000,
                verified: true,
            },
        );
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap().launchers, config.launchers);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        config
            .launchers
            .insert("bad id".into(), LauncherConfig::default());
        assert!(config.validate().is_err());
    }

    #[test]
    fn save_refuses_config_symlink() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::write(&target, "untouched").unwrap();
        let link = temp.path().join("config.toml");
        symlink(&target, &link).unwrap();
        assert!(Config::default().save(&link).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
    }

    #[test]
    fn complex_text_handler_is_shell_free_and_requires_one_file_argument() {
        let parsed = Config::parse(
            "[interaction.complex_text]\nprogram='custom-editor-command'\nargs=['--wait','{file}']",
        )
        .unwrap();
        assert_eq!(
            parsed.interaction.complex_text.unwrap().args,
            vec!["--wait", "{file}"]
        );
        for text in [
            "[interaction.complex_text]\nprogram='custom-editor-command'\nargs=[]",
            "[interaction.complex_text]\nprogram='custom-editor-command'\nargs=['--path={file}']",
            "[interaction.complex_text]\nprogram='sh'\nargs=['-c','edit {file}','{file}']",
        ] {
            assert!(Config::parse(text).is_err());
        }
    }
}
