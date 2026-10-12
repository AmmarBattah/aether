use super::{ModelKind, check_cancel};
use crate::voice::process::Control;
use anyhow::{Context, bail};
use sha1::{Digest, Sha1};
use std::{path::Path, time::Duration};
use tokio::{io::AsyncWriteExt, sync::watch};

const MODEL_HOST: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const USER_AGENT: &str = "aether-voice-setup";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_REDIRECTS: u8 = 5;

/// Follows up to [`MAX_REDIRECTS`] hops, re-vetting and re-pinning each
/// target - Hugging Face's `/resolve/` URLs 302 to a signed CDN location,
/// and `pinned_client` intentionally never follows a redirect on its own.
pub(super) async fn download_inner(
    url: &str,
    destination: &Path,
    cap: u64,
    sha1_hex: &str,
) -> anyhow::Result<()> {
    let mut url = url.to_owned();
    for hop in 0..=MAX_REDIRECTS {
        if !url.starts_with("https://") {
            bail!("model downloads require HTTPS");
        }
        let target = aether_net::vet(&url, aether_net::AddressPolicy::PublicOnly)
            .await
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let client = aether_net::pinned_client(&target, DOWNLOAD_TIMEOUT, USER_AGENT)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let mut response = client
            .get(target.url.clone())
            .send()
            .await
            .context("download the whisper model")?;
        if response.status().is_redirection() {
            if hop == MAX_REDIRECTS {
                bail!("too many model download redirects");
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("redirect response had no Location header")?
                .to_str()
                .context("redirect Location header is not valid UTF-8")?;
            url = target
                .url
                .join(location)
                .context("invalid redirect Location")?
                .to_string();
            continue;
        }
        response
            .error_for_status_ref()
            .context("model download failed")?;
        if response.content_length().is_some_and(|len| len > cap) {
            bail!("the server advertised a model larger than expected; refusing to download it");
        }
        let mut file = tokio::fs::File::create(destination)
            .await
            .with_context(|| format!("create {}", destination.display()))?;
        let mut hasher = Sha1::new();
        let mut total = 0_u64;
        while let Some(chunk) = response.chunk().await.context("read model bytes")? {
            total += chunk.len() as u64;
            if total > cap {
                bail!("model download exceeded its expected size cap");
            }
            hasher.update(&chunk);
            file.write_all(&chunk).await.context("write model bytes")?;
        }
        file.flush().await.context("flush downloaded model")?;
        let digest = hex::encode(hasher.finalize());
        if digest != sha1_hex {
            bail!("model checksum mismatch: expected {sha1_hex}, got {digest}; download refused");
        }
        return Ok(());
    }
    bail!("model download redirect limit exceeded")
}

pub(super) async fn download_model(
    model: ModelKind,
    destination: &Path,
    control: &mut watch::Receiver<Control>,
) -> anyhow::Result<()> {
    check_cancel(control)?;
    let url = format!("{MODEL_HOST}/ggml-{}.bin", model.stem());
    let cap = model.approx_bytes() * 5 / 4;
    tokio::select! {
        biased;
        _ = control.changed() => bail!("voice setup cancelled"),
        result = tokio::time::timeout(
            DOWNLOAD_TIMEOUT,
            download_inner(&url, destination, cap, model.sha1_hex()),
        ) => result.context("model download timed out")?,
    }
}
