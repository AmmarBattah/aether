# Local voice (interactive TUI)

Voice is optional and **off until you turn it on**. Nothing is recorded until
you press **F4**, run `/voice start`, or explicitly start a global question
reply in the opt-in mode below. Nothing is spoken until `/voice on` or
`/voice replies on` succeeds.
These are independent: turning question speech off does not stop an explicitly
started recording. Plain/line mode does not run audio tools.

Turning speech on is **remembered for the next launch** and restored only after
revalidation; see [Remembered settings](#remembered-settings). The default for a
new install is still off, and no recording setting is ever remembered.

Audio runs on the machine **running the TUI**, not necessarily the machine
holding your terminal window or headphones. An SSH session does not forward
microphone/headphone audio. Use a locally running TUI for local headphones.

## Use it

1. Run **`/voice setup`**. The view is the whole control panel: it
   shows what is installed, what is on in this run and what the next launch will
   restore; **Enter** on a settings row toggles question speech or replies, and
   **Enter** on a backend row installs it after one explicit confirmation. It
   installs the selected recognition engine or Piper speech output and writes
   `voice.toml` beside `ui.toml`. No microphone is opened during setup, and
   installing never turns speech on.
   Use `/voice status` to inspect the current state. Manual configuration below remains supported.
2. Run `/voice check`. This only checks local paths/configuration; it does not
   load an inference model, open a microphone, or play sound.
3. View an agent. Press **F4**, speak, then **F4** again. The TUI shows
   recording/transcribing state. A successful transcription inserts a **draft**
   at your cursor; review it and press Enter yourself. `/voice cancel` discards
   the recording or recovered draft. Recording stops automatically after 60s.
4. Run `/voice on` to speak **new** question notifications, including the asking
   agent's name. `/voice mute` or `/voice off` interrupts speech and drops the
   queue. Neither starts the microphone. `/voice off` also clears the remembered
   opt-in; `/voice mute` is transient and lasts only this run. Do not enable
   speech where question content could disclose private information to someone
   nearby.

F4 and F5 are unmodified only; they do not intercept an open view or popup. `/voice start` and `/voice stop` also work. Typing a
control command changes the composer: the resulting transcript is retained
locally with a clear `/voice insert` notice instead of overwriting your text.
The same recovery applies if you edit the draft or select another agent while
transcription runs. View the **original agent**, then **F5** to insert
at the current cursor without replacing your draft. `/voice insert` is also
available when the composer is empty; F5 preserves a nonempty draft. Cancel/off/mute, a new
recording, disconnection, and exit clear this ephemeral recovery buffer.

### When a backend is missing

Turning on question speech needs Piper speech output; turning on replies needs
Piper and a recognition engine. When one of those was never installed (its path
is not set in `voice.toml`, or there is no `voice.toml` yet), nothing is turned
on and the notice names each missing backend and what to do. With no
`voice.toml`, replies report both recognition and Piper at once and the
popup leads to recognition first. A question-speech check, for example:

```text
question speech stays off: speech output (Piper) is not installed: press Enter
on the "Install piper" row in /voice to install it, or set piper and piper_model
in voice.toml yourself (docs/voice.md)
```

If this happens inside the `/voice` popup, the selection also moves to that
backend's install row (`Install base` when no recognition engine is installed at
all), so the next **Enter** opens its confirmation. That is all it does: nothing
is downloaded until you confirm there, installing still never turns speech on,
and you can move the selection away. A typed `/voice on` or `/voice replies on`
outside the popup only shows the notice. A path that **is** set but wrong (a
moved binary, a missing model, a non-executable file) is not something the
installer fixes, so it keeps its exact error, such as
`voice configuration failed: piper: /opt/piper/piper: No such file or directory (os error 2)`;
correct the path in `voice.toml` or install the backend from the popup.

`/voice status` shows status/help. `/voice check` (also `/voice reload`) checks the current
file. Each new audio operation reads it again; an already running operation
keeps its configuration. Installation requires explicit confirmation in the
setup popup; inference never automatically downloads models or falls back to
another backend or a cloud service.

## Reply from another desktop app

This is a **separate, explicit auto-send mode**. Normal **F4** still creates a
reviewable draft; it never becomes an auto-send shortcut.

1. Install both speech recognition and Piper speech output from the `/voice`
   popup (or configure them manually below), then run **`/voice replies on`** in
   the native, interactive TUI. Enabling it checks configuration, not microphone
   or headphone routing. Like question speech it is remembered, and a restored
   opt-in still never starts a recording by itself.
2. Bind a desktop keyboard shortcut to the absolute installed executable path,
   for example **`/home/me/.local/bin/aether voice reply`**. Aether does not
   register or take over a global key itself.
3. After an agent's new question finishes playing, tap your shortcut within two
   minutes to record a reply.
   Tap it again to **stop, transcribe, and send without review** to that question's
   agent. Selecting a different agent or editing the composer does not change
   the destination or overwrite your draft.
4. Bind a second shortcut to **`aether voice cancel`** to discard a recording
   rather than send. `/voice replies off` disables this mode and stops
   remembering it, without touching question speech; `/voice off` clears both.
   `/voice mute`, suspending the TUI, a lost daemon connection and a failed
   control listener disarm the mode for this run only and leave your saved
   settings alone. Cancellation cannot retract a message already submitted to
   the daemon.

Use a modified shortcut such as **Ctrl+Super+F5** on Linux rather than bare F5,
which many applications use for refresh. You may choose F5 if your desktop
allows it, but it can steal that key from other applications. Tap, do not hold:
repeated shortcut launches are debounced. Existing in-TUI F5 draft recovery
remains available when the desktop has not captured that key.

No pending eligible question means no recording. The destination is pinned when
recording starts; a new question cannot retarget an in-flight reply. The daemon
also checks the exact question atomically at delivery and refuses an answered,
superseded, cancelled, or unavailable target. Recording timeout, transcription
failure, or stale-question rejection does not silently send a different reply.
Inspect the TUI notice/status if a shortcut appears to do nothing; desktop-launched
commands generally have no visible terminal for error output.

Keep the native TUI running. Commands do not start another TUI or daemon.
For a nondefault daemon, include its matching `--socket /absolute/path` in both
launch commands. Only one TUI can own reply control for a daemon socket at a
time; another TUI cannot steal an active listener. This is local Unix IPC,
not an SSH audio bridge or a network-accessible microphone endpoint.

### Diagnose replies without toggling the microphone

Run **`aether voice status`** (or **`aether voice status --json`**) from a shell,
using the same `--socket`/`AETHER_SOCKET` as the TUI. **`/voice status`** inside
that TUI includes the same reply diagnostic plus current/saved preferences.
Status does not enable replies, run configuration checks, replay a question,
start/stop/cancel recording, authorize delivery, or reset the shortcut debounce.

Eligibility explains the next action: `no_pending_question` means no pending
`ask_user`; `not_spoken` means no eligible newly spoken question; `synthesizing`,
`speech_queued`, and `playback_pending` mean wait for successful playback.
`speech_failed`/`playback_failed` point to the TUI and audio configuration.
`ready` permits recording; `expired` means the 120-second start window elapsed.
`target_changed`, `consumed`, or `cancelled` means answer in the TUI or wait for
a new question, not repeat the shortcut to replay/rearm the old question.
`disabled` and `busy_dictation` distinguish opt-in from F4 owning the microphone.
When the last attempt to turn speech or replies on found a backend missing,
`disabled` says which one and where to install it, qualified as **at last
check**. After editing `voice.toml`, run `/voice replies on` to re-check it.
Status stays read-only and never opens files, devices or processes; the
cached diagnosis clears after a check passes or an install finishes.
It names only the kind of backend, never a path.

Reply/cancel acknowledgements report the phase, not generic success:
- `starting`: capture requested; microphone not yet confirmed.
- `recording`: the recorder has produced audio (not proof of correct routing or speech).
- `stopping`: explicit stop/send requested; transcription has not started yet.
- `transcribing`: processing audio; cancel can still discard it.
- `sending`: delivery requested; not yet confirmed, and cannot be retracted.
- `cancelling`: discard requested; cleanup is still pending.

Status retains **one last terminal outcome and its age** for this TUI run,
separate from current eligibility: `sent`, `capture_failed`,
`transcription_failed`, `empty_transcription`, `cancelled`, `target_changed`, `discarded`, or
`delivery_unconfirmed_check_TUI`. An unconfirmed delivery is not retried; inspect
the conversation. Raw errors remain in the TUI; the bounded control response
contains no transcript, question content, or backend error details. JSON keeps
the existing `{ "ok": true, "message": "..." }` shape; `ok` means the query or
command was handled, **not** that a reply was delivered. Ineligible status
queries succeed; rejected reply commands fail.

The listener exists only while replies are enabled. A missing listener cannot
distinguish a closed TUI, disabled replies, or an unavailable endpoint; inspect
`/voice status` in the matching TUI. Status never enables a listener. If an older
TUI rejects the new status action, restart it with the updated binary.

### Ubuntu and Arch with GNOME

Open **Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts**.
Add “Aether voice reply”, enter the absolute command above, and choose a shortcut.
Repeat for cancellation. GNOME supports custom shortcuts that launch commands in
both its Wayland and X11 sessions. See the
[GNOME shortcut instructions](https://help.gnome.org/gnome-help/keyboard-shortcuts-set).

### Arch with KDE Plasma or another desktop

In KDE's application launcher, choose **Edit Applications**, create an Aether
voice-reply entry, set **Program** to the executable's absolute path and arguments
to `voice reply`, and assign **Advanced → Current shortcut key**. Do not enable
“run in terminal”. Repeat for `voice cancel`. This uses KDE's global application
shortcuts on Wayland or X11; see the
[KMenuEdit handbook](https://docs.kde.org/trunk_kf6/en/kmenuedit/kmenuedit/kmenuedit.pdf).
Other Arch desktops/window managers need their own application-shortcut binding
to the same command; there is no universal Arch shortcut settings screen.

### macOS

In **Shortcuts**, create “Aether Voice Reply”, add **Run Shell Script** with the
absolute command, then use **Details → Add Keyboard Shortcut**. Create a separate
cancel shortcut. Apple supports invoking these shortcuts from other apps, except
for reserved system shortcuts: see
[Shortcuts keyboard bindings](https://support.apple.com/en-au/guide/shortcuts-mac/apd163eb9f95/mac).
A modified chord such as Control+Option+Command+F5 avoids common application keys.
Depending on keyboard settings, the function key may require Fn/Globe; see
[Apple's function-key settings](https://support.apple.com/guide/mac-help/use-keyboard-function-keys-mchlp2596/mac).

Allow microphone access for the terminal hosting the TUI when macOS requests it.
Aether does not need Accessibility/Input Monitoring permission to receive global
keystrokes: the OS shortcut invokes a command instead. Select your headphones
as the output device; Aether cannot prevent the OS switching to speakers after
headphones disconnect. These bindings target normal apps in your logged-in
session, not login/lock screens or reserved secure-input contexts. Real-device
microphone/headphone and desktop-shortcut behavior must be checked on your host;
unit tests do not establish hardware compatibility.

## Remembered settings

`/voice on`, `/voice off`, `/voice replies on`, `/voice replies off` and the
popup's settings rows write question speech and reply mode to `ui.toml`, beside
your other UI preferences, which are preserved. Replies imply question speech:
they are never saved on their own. Nothing else about voice is persisted — an
open microphone, a pending question, a recovered draft, a mute and a cancelled
install all end with the session.

A saved opt-in is **restored at most once per launch**, and only in a native,
connected, interactive TUI. Restoration first repeats the same configuration
check as the command, so a missing or broken Piper leaves speech off with an
explicit notice and keeps the saved setting for a later launch. Restoring turns
on speech only: it never arms reply mode's auto-send, starts a recording, or
replays questions from before the TUI connected. A finished installation
re-checks a saved opt-in, but installing never enables something you had not
already saved.

If preferences cannot be written — a read-only or missing configuration
directory, for instance — the change still applies to this run and the notice
says so, naming what the next launch would still restore. Failures are never
silent, and the popup's "next launch" line always reports what is actually on
disk.

## Guided model installation

The popup offers different recognition families, not just model sizes:

- **Whisper tiny/base/small:** multilingual whisper.cpp models, approximately
  75/142/466 MiB respectively. Base is the balanced starting point; tiny favors
  lower resource use and small generally favors accuracy at higher cost.
- **Parakeet TDT 0.6B v3 int8:** a separate NVIDIA model run locally on CPU
  through sherpa-onnx. About 465 MiB to download and 640 MiB for model files,
  plus the Python runtime packages. It automatically recognizes
  [25 European languages](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3),
  not every language Whisper supports. Model license: CC-BY-4.0.
- **Piper speech output:** the only entry that installs the *speaking* half.
  Hash-pinned `piper-tts==1.8.0` and CPU runtime wheels from PyPI, plus the
  `en_US-ljspeech-medium` voice (~61 MiB) from a pinned Hugging Face revision of
  `rhasspy/piper-voices`. Piper is GPL-3.0-or-later; the LJ Speech dataset is
  public domain in the US. Allow ~500 MiB of disk. It writes only `piper` and
  `piper_model`, leaving transcription settings untouched, and neither records
  nor plays audio during installation.

Piper's pinned wheels support **CPython 3.11–3.14 with venv/pip**, on **Linux
glibc 2.27+ x86-64/ARM64** or **macOS 14+ Apple Silicon** only; FFmpeg is also
required. Intel macOS, musl Linux, Windows, PyPy and free-threaded interpreters
are not covered by this pin and fail with an explicit package-installation
error. Voice selection is still fixed to LJ Speech; other voices need the manual
configuration below.

Setup reports missing prerequisites rather than running `sudo` or silently
changing system packages. Whisper requires Git, CMake, a C/C++ compiler, a build
tool, and FFmpeg. Parakeet requires Python with venv/pip and FFmpeg, but no C++
compiler or GPU. The pinned Parakeet runtime is `sherpa-onnx==1.13.8` with
`numpy==2.4.3`, installed as wheels in a private environment. NumPy requires
Python 3.11+; supported wheel combinations include CPython 3.11–3.14 on Linux
x86-64/ARM64 and macOS Intel/Apple Silicon. Linux needs glibc 2.27+ for these
NumPy wheels; musl Linux is not supported by this sherpa-onnx setup. Older
OS/Python combinations can fail with an explicit package-installation error.

After confirmation, setup uses the network for GitHub/Hugging Face model
downloads and, for Parakeet and Piper, PyPI runtime packages. Models are
streamed with size limits and checked against pinned content hashes; the
Parakeet archive and the Piper voice use SHA-256 and Whisper models use the
upstream-published SHA-1 checksums. Only the expected regular Parakeet model
files are unpacked. Parakeet's runtime is installed as version-pinned wheels;
Piper's `requirements.txt` additionally pins every wheel hash and refuses source
builds. Treat confirmed installation as trusting those upstream publishers.

Setup replaces `voice.toml` atomically, so it refuses a `voice.toml` that is a
symbolic link rather than silently turning the link into a separate file. The
refusal names the full path and the fix:

```text
voice setup failed: /home/me/.config/aether/voice.toml is a link, and setup only
rewrites a regular file: replace the link with a copy of the file it points to,
then run the install again
```

For example `cp -L voice.toml voice.toml.copy && mv voice.toml.copy voice.toml`
in that directory, then press **Enter** on the install row again. Nothing is
downloaded or changed before this check. Reading a linked `voice.toml` for
dictation and speech keeps working; only the installer needs a regular file.

Each installation lives under `voice/` beside `voice.toml`. Setup preserves the
settings it does not own — installing a recognition backend leaves Piper alone
and installing Piper leaves transcription alone — and rewrites `voice.toml` only
after a successful installation. Previous installations remain on disk so
a failed new attempt cannot damage a working one. Repeated installations use
additional disk space; there is not yet a cache manager or uninstall popup.
Changing or cancelling setup never sends a transcript, enables spoken questions
or changes what the next launch restores.

## Tools and models

The supported contracts are [whisper.cpp v1.9.4](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.4)
and [OHF-Voice Piper v1.8.0](https://github.com/OHF-Voice/piper1-gpl/releases/tag/v1.8.0),
plus an FFmpeg build with the device support below. These are independent local
executables, not libraries linked into Aether. Newer versions may work but are
not automatically selected or installed.

### Transcription

[whisper.cpp](https://github.com/ggml-org/whisper.cpp) is MIT-licensed, supports
CPU inference and Apple Silicon acceleration, and builds with CMake and a C++
toolchain. Its published model estimates are: tiny 75MiB disk/~273MB memory;
base 142MiB/~388MB; small 466MiB/~852MB. Larger models need more memory and can
exceed Aether's 120s inference timeout. Latency depends on hardware; no GPU is
required. Choose multilingual `base` or `tiny`; `.en` models are English-only.

The following are **manual installation commands that use the network**, not
commands Aether runs. Choose an installation directory you own:

```sh
git clone --branch v1.9.4 --depth 1 https://github.com/ggml-org/whisper.cpp.git
cd whisper.cpp
cmake -B build
cmake --build build --config Release -j2
sh models/download-ggml-model.sh base
```

Set `whisper` to the absolute `build/bin/whisper-cli` path and `whisper_model`
to the downloaded `models/ggml-base.bin`. Set `language = "en"` for English-only
models or a known language; `"auto"` is the multilingual default.

[faster-whisper](https://github.com/SYSTRAN/faster-whisper) is another MIT local
Whisper implementation, using Python/CTranslate2. We chose whisper.cpp's direct
file-to-text CLI rather than adding a Python ASR adapter, model-resolution layer,
or service. It is not offered in the setup picker; Parakeet is the supported
alternative to whisper.cpp.

### Parakeet (manual configuration)

The guided installer handles the separate CPU runtime, the
[sherpa-onnx ONNX export](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html#sherpa-onnx-nemo-parakeet-tdt-0-6b-v3-int8-multilingual),
and configuration. To use an existing compatible install, set:

```toml
ffmpeg = "/usr/bin/ffmpeg"
stt_backend = "parakeet"
sherpa_python = "/home/me/voice/env/bin/python"
sherpa_model_dir = "/home/me/voice/models"
language = "auto"
```

The model directory must contain `encoder.int8.onnx`, `decoder.int8.onnx`,
`joiner.int8.onnx`, and `tokens.txt` from that export. The interpreter must have
the pinned sherpa-onnx and NumPy packages. Aether supplies its own bounded WAV
adapter; it does not search for models or download anything during transcription.
The existing 60-second recording and 120-second inference limits also apply to
Parakeet. `language` must be `"auto"` for this backend.

Existing configs without `stt_backend` continue to select whisper.cpp. Set
`stt_backend = "whisper_cpp"` to switch back to a configured Whisper pair.

### Speech

**Spoken agent questions are supported, and the `/voice` popup installs Piper
for you.** Once Piper and a voice are configured, `/voice on` reads new question
notifications with the agent's name; `/voice mute` stops playback. This does not
require a new language model or a cloud service. Choose your headphones as the
OS default output, or configure `output_device` below. Aether cannot guarantee
that the OS routes audio to headphones, especially after a device disconnects.

The remaining usability work for speech is voice-license selection beyond the
bundled LJ Speech pin and a device picker with an explicit test-sound action;
both still need manual `voice.toml` edits. A persistent Piper process could
reduce repeated model-load latency, but would need its own cancellation, idle
shutdown, and memory limits; the current bounded per-question process remains
the simpler default. No real-device latency or headphone-routing benchmark is
claimed here.

[Piper's current CLI](https://github.com/OHF-Voice/piper1-gpl/blob/v1.8.0/docs/CLI.md)
is distributed as `piper-tts`, with a `piper` executable. Use the OHF successor,
not the old rhasspy binary downloads. It requires Python 3.9+, ONNX Runtime,
and its packaged espeak-ng data. The engine is GPL-3.0-or-later; **individual
voices have their own licenses**: inspect each voice's
[MODEL_CARD](https://github.com/OHF-Voice/piper1-gpl/blob/v1.8.0/docs/VOICES.md).
Nothing is bundled or relicensed by Aether. Piper currently advertises a need
for maintainers; these links are versioned contracts, not a maintenance promise.

Manual, network-using setup for a different voice or platform, from your chosen
installation directory (the `/voice` popup does the equivalent for the pinned
LJ Speech voice):

```sh
python3 -m venv piper-env
piper-env/bin/python -m pip install 'piper-tts==1.8.0'
mkdir -p voices
piper-env/bin/python -m piper.download_voices en_US-lessac-medium --data-dir voices
```

Set `piper` to the absolute `piper-env/bin/piper` path. Set `piper_model` to the
absolute `.onnx` path; its matching `.onnx.json` must remain beside it. Memory
and disk requirements vary by voice, ONNX Runtime build and sentence length;
there is no universal verified RAM budget. Each question loads the model again,
trading warm-start speed for simple ownership and no background inference server.

Only `espeak`/`text` phonemizer voices are accepted. Other Piper phonemizers can
fetch auxiliary resources; they are rejected rather than downloading anything.
Use local models and the pinned engine. Configured executables and model files
are a **local code/data trust boundary**, not an Aether network sandbox.

## Devices and configuration

Install FFmpeg through a trusted package manager or the
[official distribution guidance](https://ffmpeg.org/download.html).
[FFmpeg licensing](https://ffmpeg.org/legal.html) depends on build options
(LGPL by default, GPL components possible).

Device support is defined in the [official device manual](https://ffmpeg.org/ffmpeg-devices.html):

- **Linux:** FFmpeg with PulseAudio input **and** output (`--enable-libpulse`),
  using a local PulseAudio server or PipeWire's PulseAudio compatibility service.
  Choose a microphone source and headphone sink with `pactl list short sources`
  and `pactl list short sinks`. Default source/sink are used when omitted.
- **macOS:** FFmpeg with **AVFoundation input** and **AudioToolbox output**.
  Merely having an AudioToolbox codec is insufficient. `ffmpeg -devices` must
  list the respective input/output devices. Microphone permission is required.
  `input_device = ":0"` selects audio input index 0, with no video input;
  `output_device = "2"` selects CoreAudio output index 2. Omit output_device to
  use the OS default. Availability depends on your particular FFmpeg build.
- Windows and other platforms: this client integration is not supported, even
  though the inference engines themselves may support them. Piper's published
  [wheel workflow](https://github.com/OHF-Voice/piper1-gpl/blob/v1.8.0/.github/workflows/publish.yml)
  includes Linux x86-64/ARM64 and Intel/ARM macOS; Python/platform wheel support
  still depends on the package dependencies (notably no musl wheels in that workflow).

Example `voice.toml` (replace all paths; `~` is not expanded):

```toml
ffmpeg = "/usr/bin/ffmpeg"
whisper = "/home/me/voice/whisper.cpp/build/bin/whisper-cli"
whisper_model = "/home/me/voice/whisper.cpp/models/ggml-base.bin"
language = "auto"
piper = "/home/me/voice/piper-env/bin/piper"
piper_model = "/home/me/voice/voices/en_US-lessac-medium.onnx"
# Optional Linux source/sink names:
# input_device = "alsa_input.your_microphone"
# output_device = "alsa_output.your_headphones"
# Optional Linux local socket override:
# pulse_socket = "/run/user/1000/pulse/native"
```

Configure just the whisper pair or just the Piper pair if you only need one
direction. Linux explicitly connects to the local Unix socket
`$XDG_RUNTIME_DIR/pulse/native` (or `/run/user/<uid>/pulse/native`); it does not
inherit a remote `PULSE_SERVER`. Your local audio server/OS still controls actual
hardware routing. **Headphone routing has not been verified by Aether.**

## Limits, privacy, and verification

Capture: mono 16kHz signed 16-bit PCM, at most 60s/~1.92MB, followed by one private
0600 temporary WAV, deleted after transcription/cancellation. Synthesis: question
text including agent label at most 2048 UTF-8 bytes, at most 90s PCM (8.64MB at
48kHz). Inference/playback: 120s timeout each. Transcript: 16KiB UTF-8, no terminal
control bytes; Unicode content is unchanged by ASCII UI mode. Stderr is bounded
to 16KiB, never logged; failures report stage/exit status. There is one capture,
one active speech job and at most three queued questions. Queued/pre-playback
questions expire after 30s and are checked against live, unanswered cards.
Replay/reconnect backlogs are never spoken. Starting a recording interrupts
speech and drops queued notifications to avoid feedback.

No shell interpolation, server, cloud fallback, or automatic microphone listening
is used. Text/PCM travel through bounded stdin/stdout or the private WAV, not
question-derived command options. The interactive runner cancels process groups
and reaps owned children on disconnect, cancellation, external-command handoff,
and exit. There is no claim that OS kill cannot fail, crashes securely erase RAM,
or SIGKILL/power loss can clean a temporary file. Use an encrypted local temporary
directory when your threat model requires it. Recovery text is memory-only, not
an agent message until you explicitly send the reviewed draft.

The default automated tests use **fake local processes**, never a real microphone,
speakers, cloud API, or downloaded model. They verify lifecycle, bounds, failure
and cancellation handling, process cleanup, configuration, replay suppression,
question freshness, draft isolation, saved-setting persistence and restoration,
and Unicode/ASCII behavior. Real-model accuracy/latency and actual Linux/macOS
audio hardware need separate verification.

The ignored `real_piper_install_and_synthesis_without_playback` test runs the
real Piper installer into the worktree's `target/piper-smoke` and synthesizes
PCM without opening any audio device; run it with
`cargo test -p aether-tui real_piper_install_and_synthesis -- --ignored --nocapture`.

Ignored `real_whisper_install_and_transcription` and
`real_parakeet_install_and_transcription` tests exercise the real network installer
and transcribe a supplied WAV without opening audio hardware. To opt in, set
`AETHER_VOICE_SMOKE_DIR` to an absolute scratch directory under the worktree's
`target/`, set `AETHER_VOICE_SMOKE_WAV` to an absolute public/test WAV path, and
run `cargo test -p aether-tui real_whisper_install_and_transcription -- --ignored --nocapture`
(or the Parakeet test name). Successful smoke installations are retained there
for inspection; they do not modify your normal Aether configuration.
