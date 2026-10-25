#!/usr/bin/env python3
"""Regenerate synthetic retained-seek fixtures; tests do not require FFmpeg."""
from pathlib import Path
import hashlib
import math
import struct
import subprocess
import wave

root = Path(__file__).resolve().parents[1]
out = root / 'crates/deadlib-audio-decode/tests/fixtures'
out.mkdir(parents=True, exist_ok=True)
source = out / 'seek.wav'
with wave.open(str(source), 'wb') as wav:
    wav.setparams((2, 2, 44100, 0, 'NONE', 'not compressed'))
    pcm = bytearray()
    for frame in range(39690):
        t = frame / 44100
        left = int(10000 * math.sin(2 * math.pi * (180 * t + 140 * t * t)))
        right = int(7000 * math.sin(2 * math.pi * (270 * t + 90 * t * t)))
        pcm.extend(struct.pack('<hh', left, right))
    wav.writeframes(pcm)
for suffix, args in [
    ('flac', ['-c:a', 'flac']),
    ('ogg', ['-c:a', 'vorbis', '-strict', 'experimental', '-q:a', '4']),
    ('opus', ['-c:a', 'libopus', '-b:a', '96k']),
    ('mp3', ['-c:a', 'libmp3lame', '-b:a', '96k', '-id3v2_version', '0', '-write_id3v1', '0']),
]:
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-fflags', '+bitexact', '-i', str(source),
                    '-map_metadata', '-1', '-threads', '1', *args, str(out / ('seek.' + suffix))], check=True)
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-fflags', '+bitexact', '-stream_loop', '2',
                '-i', str(source), '-t', '2.4', '-map_metadata', '-1', '-threads', '1',
                '-c:a', 'libopus', '-b:a', '96k', str(out / 'seek-long.opus')], check=True)
for path in sorted(out.glob('seek*')):
    if path.suffix not in {'.wav', '.flac', '.ogg', '.opus', '.mp3'}:
        continue
    print(hashlib.sha256(path.read_bytes()).hexdigest(), path.name, path.stat().st_size)
