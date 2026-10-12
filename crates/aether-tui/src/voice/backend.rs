use std::io::Write;
use std::time::Duration;

use anyhow::{Context, bail};
use tokio::process::Command;
use tokio::sync::watch;

use super::config::{Config, required};
use super::process::{self, Control};

pub const TEXT_LIMIT: usize = 16_384;
pub const QUESTION_LIMIT: usize = 2_048;
const RECORD_SECONDS: u64 = 60;
const INFERENCE_TIMEOUT: Duration = Duration::from_secs(120);

fn ffmpeg(config: &Config) -> Command {
    let mut command = Command::new(&config.ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    command
}

pub async fn record(
    config: &Config,
    control: &mut watch::Receiver<Control>,
) -> anyhow::Result<tempfile::NamedTempFile> {
    record_inner(config, control, None).await
}

pub async fn record_reply(
    config: &Config,
    control: &mut watch::Receiver<Control>,
    on_audio: impl FnOnce() + Send + 'static,
) -> anyhow::Result<tempfile::NamedTempFile> {
    record_inner(config, control, Some(Box::new(on_audio))).await
}

async fn record_inner(
    config: &Config,
    control: &mut watch::Receiver<Control>,
    on_audio: Option<Box<dyn FnOnce() + Send>>,
) -> anyhow::Result<tempfile::NamedTempFile> {
    config.recording()?;
    let mut command = ffmpeg(config);
    if cfg!(target_os = "macos") {
        command.args([
            "-f",
            "avfoundation",
            "-i",
            config.input_device.as_deref().unwrap_or(":default"),
        ]);
    } else {
        command.args(["-server", &config.pulse_server()?]);
        command.args([
            "-f",
            "pulse",
            "-i",
            config.input_device.as_deref().unwrap_or("default"),
        ]);
    }
    command.args([
        "-vn",
        "-ar",
        "16000",
        "-ac",
        "1",
        "-c:a",
        "pcm_s16le",
        "-f",
        "s16le",
        "pipe:1",
    ]);
    let limit = RECORD_SECONDS as usize * 32_000;
    let timeout = Duration::from_secs(RECORD_SECONDS);
    let mut pcm = if let Some(on_audio) = on_audio {
        process::run_explicit_recording(command, limit, timeout, control, on_audio).await?
    } else {
        process::run(command, &[], limit, timeout, control, true).await?
    };
    // A stopped raw stream can end halfway through its final sample.
    pcm.truncate(pcm.len() / 2 * 2);
    if pcm.len() < 3_200 {
        bail!(
            "recording contained less than 100ms of audio; check input_device and microphone permission"
        );
    }
    let mut file = tempfile::Builder::new()
        .prefix("aether-voice-")
        .suffix(".wav")
        .tempfile()?;
    write_wav(&mut file, &pcm, 16_000)?;
    file.flush()?;
    Ok(file)
}

fn write_wav(file: &mut impl Write, pcm: &[u8], rate: u32) -> anyhow::Result<()> {
    let size = u32::try_from(pcm.len()).context("audio exceeds WAV size")?;
    file.write_all(b"RIFF")?;
    file.write_all(&(size + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * 2).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&size.to_le_bytes())?;
    file.write_all(pcm)?;
    Ok(())
}

pub async fn transcribe(
    config: &Config,
    file: &std::path::Path,
    control: &mut watch::Receiver<Control>,
) -> anyhow::Result<String> {
    if config.stt_backend == super::config::SttBackend::Parakeet {
        return super::sherpa::transcribe(config, file, control).await;
    }
    let mut command = Command::new(required(&config.whisper, "whisper")?);
    command
        .arg("-m")
        .arg(required(&config.whisper_model, "whisper_model")?)
        .arg("-f")
        .arg(file)
        .args(["-l", &config.language, "-nt", "-np", "-t", "4"]);
    let bytes = process::run(command, &[], TEXT_LIMIT, INFERENCE_TIMEOUT, control, false).await?;
    transcript(bytes)
}

pub fn transcript(bytes: Vec<u8>) -> anyhow::Result<String> {
    let text = String::from_utf8(bytes).context("transcription is not UTF-8")?;
    if text.len() > TEXT_LIMIT
        || text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        bail!("transcription contains invalid control characters or exceeds 16384 bytes");
    }
    let text = text.trim().to_owned();
    if text.is_empty() {
        bail!("no speech recognized; draft unchanged");
    }
    Ok(text)
}

pub async fn synthesize(
    config: &Config,
    text: &str,
    control: &mut watch::Receiver<Control>,
) -> anyhow::Result<(Vec<u8>, u32)> {
    if text.len() > QUESTION_LIMIT || text.is_empty() {
        bail!("spoken question must contain 1..2048 bytes");
    }
    let rate = config.speech()?;
    let mut command = Command::new(required(&config.piper, "piper")?);
    command
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1");
    command
        .arg("--model")
        .arg(required(&config.piper_model, "piper_model")?)
        .arg("--output-raw");
    let pcm = process::run(
        command,
        text.as_bytes(),
        rate as usize * 2 * 90,
        INFERENCE_TIMEOUT,
        control,
        false,
    )
    .await?;
    if pcm.is_empty() || pcm.len() % 2 != 0 {
        bail!("Piper returned empty or incomplete 16-bit PCM audio");
    }
    Ok((pcm, rate))
}

pub async fn play(
    config: &Config,
    pcm: &[u8],
    rate: u32,
    control: &mut watch::Receiver<Control>,
) -> anyhow::Result<()> {
    let mut command = ffmpeg(config);
    command.args([
        "-f",
        "s16le",
        "-ar",
        &rate.to_string(),
        "-ac",
        "1",
        "-i",
        "pipe:0",
    ]);
    if cfg!(target_os = "macos") {
        command.args(["-f", "audiotoolbox"]);
        if let Some(device) = &config.output_device {
            command.args(["-audio_device_index", device]);
        }
        command.arg("-");
    } else {
        command.args(["-f", "pulse"]);
        command.args(["-server", &config.pulse_server()?]);
        if let Some(device) = &config.output_device {
            command.args(["-device", device]);
        }
        command.arg("Aether question");
    }
    process::run(command, pcm, 1_024, INFERENCE_TIMEOUT, control, false).await?;
    Ok(())
}
