"""Bounded local Parakeet inference and allowlisted model unpacking."""

import shutil
import sys
import tarfile
import wave
from pathlib import Path


MODEL_FILES = {
    "encoder.int8.onnx": 700 * 1024 * 1024,
    "decoder.int8.onnx": 20 * 1024 * 1024,
    "joiner.int8.onnx": 10 * 1024 * 1024,
    "tokens.txt": 1024 * 1024,
}
ARCHIVE_ROOT = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"


def unpack(archive_path, destination):
    destination.mkdir()
    found = set()
    with tarfile.open(archive_path, "r|bz2") as archive:
        for index, member in enumerate(archive):
            if index >= 128:
                raise ValueError("too many entries in Parakeet archive")
            name = member.name.removeprefix("./")
            prefix = ARCHIVE_ROOT + "/"
            if not name.startswith(prefix):
                continue
            filename = name[len(prefix):]
            if filename not in MODEL_FILES:
                continue
            if filename in found or not member.isfile():
                raise ValueError("duplicate or non-regular model file")
            if not 0 < member.size <= MODEL_FILES[filename]:
                raise ValueError("model file exceeds expected size")
            source = archive.extractfile(member)
            if source is None:
                raise ValueError("missing model content")
            with source, (destination / filename).open("xb") as output:
                shutil.copyfileobj(source, output, length=64 * 1024)
            found.add(filename)
    if found != set(MODEL_FILES):
        raise ValueError("Parakeet archive is missing required model files")


def transcribe(model_dir, audio_path):
    import numpy as np
    import sherpa_onnx

    with wave.open(str(audio_path), "rb") as audio:
        frames = audio.getnframes()
        if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate()) != (1, 2, 16000):
            raise ValueError("expected mono 16kHz PCM16 WAV")
        if not 0 < frames <= 60 * 16000:
            raise ValueError("audio must contain at most 60 seconds")
        pcm = audio.readframes(frames)
        if len(pcm) != frames * 2:
            raise ValueError("truncated WAV")
    samples = np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768.0
    recognizer = sherpa_onnx.OfflineRecognizer.from_transducer(
        encoder=str(model_dir / "encoder.int8.onnx"),
        decoder=str(model_dir / "decoder.int8.onnx"),
        joiner=str(model_dir / "joiner.int8.onnx"),
        tokens=str(model_dir / "tokens.txt"),
        num_threads=2, sample_rate=16000, feature_dim=80,
        decoding_method="greedy_search", model_type="nemo_transducer",
        debug=False, provider="cpu",
    )
    stream = recognizer.create_stream()
    stream.accept_waveform(16000, samples)
    recognizer.decode_stream(stream)
    print(stream.result.text, flush=True)


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise ValueError("expected action, model/archive, and audio/destination")
    if sys.argv[1] == "unpack":
        unpack(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == "transcribe":
        transcribe(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        raise ValueError("unknown action")
