use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::ModelKind;

pub const FILE: &str = "voice.toml";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SttBackend {
    #[default]
    WhisperCpp,
    Parakeet,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub ffmpeg: PathBuf,
    #[serde(default)]
    pub stt_backend: SttBackend,
    pub sherpa_python: Option<PathBuf>,
    pub sherpa_model_dir: Option<PathBuf>,
    pub whisper: Option<PathBuf>,
    pub whisper_model: Option<PathBuf>,
    pub piper: Option<PathBuf>,
    pub piper_model: Option<PathBuf>,
    #[serde(default = "language")]
    pub language: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub pulse_socket: Option<PathBuf>,
}

fn language() -> String {
    "auto".into()
}

impl Config {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(FILE);
        let bytes = read_small(&path, 16_384).with_context(|| {
            format!(
                "run /voice setup or configure {}; see docs/voice.md",
                path.display()
            )
        })?;
        let text = std::str::from_utf8(&bytes).context("voice.toml must be UTF-8")?;
        toml::from_str(text).context("invalid voice.toml")
    }

    /// `load` for a check that needs `missing`: with no voice.toml at all,
    /// nothing was ever installed, so that backend is the thing to install.
    pub fn load_needing(dir: &Path, missing: Missing) -> anyhow::Result<Self> {
        match std::fs::symlink_metadata(dir.join(FILE)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(missing.into()),
            _ => Self::load(dir),
        }
    }

    pub fn load_replies(dir: &Path) -> anyhow::Result<Self> {
        let recognition = Missing::Recognition(SttBackend::default());
        let config = Self::load_needing(dir, recognition).map_err(|error| {
            if error.downcast_ref::<Missing>().is_some() {
                error.context(Missing::Speech.to_string())
            } else {
                error
            }
        })?;
        let recording = config.recording();
        let speech = config.speech();
        match (recording, speech) {
            (Err(recording), Err(speech))
                if recording.downcast_ref::<Missing>().is_some()
                    && speech.downcast_ref::<Missing>().is_some() =>
            {
                Err(recording.context(format!("{speech:#}")))
            }
            // Partial/broken configurations keep their established STT-first failure.
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(()), Ok(_)) => Ok(config),
        }
    }

    pub fn recording(&self) -> anyhow::Result<()> {
        platform()?;
        if cfg!(target_os = "linux") {
            self.pulse_server()?;
        }
        executable(&self.ffmpeg, "ffmpeg")?;
        match self.stt_backend {
            SttBackend::WhisperCpp => {
                let whisper = installed(&self.whisper, Missing::Recognition(self.stt_backend))?;
                executable(whisper, "whisper")?;
                local_file(
                    required(&self.whisper_model, "whisper_model")?,
                    "whisper_model",
                )?;
            }
            SttBackend::Parakeet => {
                let python =
                    installed(&self.sherpa_python, Missing::Recognition(self.stt_backend))?;
                executable(python, "sherpa_python")?;
                let dir = required(&self.sherpa_model_dir, "sherpa_model_dir")?;
                for name in super::sherpa::MODEL_FILES {
                    local_file(&dir.join(name), "Parakeet model file")?;
                }
                if self.language != "auto" {
                    bail!("Parakeet detects language automatically; set language = \"auto\"");
                }
            }
        }
        if self.language.is_empty()
            || self.language.starts_with('-')
            || self.language.len() > 16
            || !self
                .language
                .bytes()
                .all(|c| c.is_ascii_alphabetic() || c == b'-')
        {
            bail!("language must be a Whisper language code or auto");
        }
        device(self.input_device.as_deref(), "input_device")?;
        if cfg!(target_os = "macos")
            && self
                .input_device
                .as_ref()
                .is_some_and(|d| !d.starts_with(':') || d.len() < 2)
        {
            bail!("macOS input_device must be :<audio name/index>; video capture is not supported");
        }
        Ok(())
    }

    pub fn speech(&self) -> anyhow::Result<u32> {
        platform()?;
        if cfg!(target_os = "linux") {
            self.pulse_server()?;
        }
        executable(&self.ffmpeg, "ffmpeg")?;
        executable(installed(&self.piper, Missing::Speech)?, "piper")?;
        let model = required(&self.piper_model, "piper_model")?;
        local_file(model, "piper_model")?;
        device(self.output_device.as_deref(), "output_device")?;
        if cfg!(target_os = "macos")
            && self
                .output_device
                .as_ref()
                .is_some_and(|d| d.parse::<u32>().is_err())
        {
            bail!("macOS output_device must be a CoreAudio device index");
        }
        let mut json = model.as_os_str().to_owned();
        json.push(".json");
        let config: serde_json::Value =
            serde_json::from_slice(&read_small(Path::new(&json), 1_048_576)?)
                .context("invalid Piper .onnx.json")?;
        if config
            .get("phoneme_type")
            .is_some_and(|kind| kind != "espeak" && kind != "text")
        {
            bail!(
                "only Piper espeak/text voices are supported offline; other phonemizers may download resources"
            );
        }
        let rate = config["audio"]["sample_rate"]
            .as_u64()
            .filter(|r| (8_000..=48_000).contains(r))
            .context("Piper .onnx.json requires audio.sample_rate between 8000 and 48000")?;
        Ok(rate as u32)
    }

    pub fn pulse_server(&self) -> anyhow::Result<String> {
        let path = self.pulse_socket.clone().unwrap_or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", nix::unistd::getuid())))
                .join("pulse/native")
        });
        if !path.is_absolute()
            || path
                .to_str()
                .is_none_or(|s| s.chars().any(|c| c.is_control() || c.is_whitespace()))
        {
            bail!("pulse_socket must be an absolute UTF-8 local Unix socket path");
        }
        Ok(format!("unix:{}", path.display()))
    }
}

fn device(value: Option<&str>, name: &str) -> anyhow::Result<()> {
    if value.is_some_and(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control)) {
        bail!("{name} must be a nonempty local device name (at most 256 bytes)");
    }
    Ok(())
}

/// A backend the `/voice` installer provides that `voice.toml` does not name.
/// A path that is set but broken is never this: it keeps its precise error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// Piper speech output.
    Speech,
    /// The recognition engine `stt_backend` selects.
    Recognition(SttBackend),
}

impl Missing {
    /// The `/voice` install row that provides it.
    pub fn install_row(self) -> ModelKind {
        match self {
            Self::Speech => ModelKind::Piper,
            Self::Recognition(SttBackend::Parakeet) => ModelKind::Parakeet,
            Self::Recognition(SttBackend::WhisperCpp) => ModelKind::Base,
        }
    }

    /// The same guidance for `aether voice status`, which has no popup.
    pub fn reply_diagnostic(self) -> &'static str {
        match self {
            Self::Speech => {
                "disabled: speech output (Piper) is not installed at last check; open /voice, press Enter on the \"Install piper\" row; after voice.toml edits re-check /voice replies on"
            }
            Self::Recognition(SttBackend::Parakeet) => {
                "disabled: speech recognition (Parakeet) is not installed at last check; open /voice, press Enter on the \"Install parakeet\" row; after voice.toml edits re-check /voice replies on"
            }
            Self::Recognition(SttBackend::WhisperCpp) => {
                "disabled: speech recognition is not installed at last check; open /voice, press Enter on a recognition install row (base); after voice.toml edits re-check /voice replies on"
            }
        }
    }
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let row = self.install_row().label();
        match self {
            Self::Speech => write!(
                f,
                "speech output (Piper) is not installed: press Enter on the \"Install {row}\" row in /voice to install it, or set piper and piper_model in voice.toml yourself (docs/voice.md)"
            ),
            Self::Recognition(SttBackend::Parakeet) => write!(
                f,
                "speech recognition (Parakeet) is not installed: press Enter on the \"Install {row}\" row in /voice to install it, or set sherpa_python and sherpa_model_dir in voice.toml yourself (docs/voice.md)"
            ),
            Self::Recognition(SttBackend::WhisperCpp) => write!(
                f,
                "speech recognition is not installed: press Enter on a recognition row in /voice to install one (\"Install {row}\" is the default), or set whisper and whisper_model in voice.toml yourself (docs/voice.md)"
            ),
        }
    }
}

impl std::error::Error for Missing {}

/// The path of an installable backend, or which backend to install.
fn installed(value: &Option<PathBuf>, missing: Missing) -> anyhow::Result<&Path> {
    value.as_deref().ok_or_else(|| missing.into())
}

pub fn required<'a>(value: &'a Option<PathBuf>, name: &str) -> anyhow::Result<&'a Path> {
    value
        .as_deref()
        .with_context(|| format!("set {name} in voice.toml; see docs/voice.md"))
}

fn local_file(path: &Path, name: &str) -> anyhow::Result<()> {
    if !path.is_absolute() {
        bail!("{name} must be an absolute local path (no ~ expansion)");
    }
    let meta = path
        .metadata()
        .with_context(|| format!("{name}: {}", path.display()))?;
    if !meta.is_file() || meta.len() == 0 {
        bail!("{name} must be a nonempty regular file: {}", path.display());
    }
    Ok(())
}

fn executable(path: &Path, name: &str) -> anyhow::Result<()> {
    local_file(path, name)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.metadata()?.permissions().mode() & 0o111 == 0 {
            bail!("{name} is not executable: {}", path.display());
        }
    }
    Ok(())
}

pub fn read_small(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    if !file.metadata()?.is_file() {
        bail!("expected a regular file: {}", path.display());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!("file exceeds {limit} bytes: {}", path.display());
    }
    Ok(bytes)
}

pub fn platform() -> anyhow::Result<()> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        bail!("local voice currently supports Linux and macOS interactive terminals only");
    }
    Ok(())
}
