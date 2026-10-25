`timeline.mp3` is a generated 2.4-second stereo chirp, containing an Info/LAME
header, encoder delay and padding. It contains no third-party recording.
Generated with FFmpeg 7.1 / libmp3lame using:

```sh
ffmpeg -f lavfi -i 'aevalsrc=0.3*sin(2*PI*(220*t+70*t*t))|0.2*sin(2*PI*(330*t+110*t*t)):s=44100:d=2.4' -c:a libmp3lame -b:a 96k -map_metadata -1 -id3v2_version 0 -write_id3v1 0 timeline.mp3
```

The MP3 tests derive Xing, ID3-prefixed and headerless variants from these same
encoded audio frames. The PCM hashes, duration bits and seek hashes were captured
from the decoder before adding timeline options. Tests do not require FFmpeg.

## Retained-seek fixtures

`seek.wav` is a generated 0.9-second 44.1 kHz stereo chirp, with independent
left/right phases and amplitudes. `seek.flac`, `seek.ogg`, `seek.opus` and
`seek.mp3` encode that same generated source. No recording or third-party music
is included. Regenerate with `python3 scripts/generate-seek-audio-fixtures.py`
from the repository root. This generation used Python 3.13 and FFmpeg 8.1.1 with
native FLAC/Vorbis, libopus and libmp3lame encoders. Tests consume committed files
and require neither Python nor FFmpeg. Encoder versions can change encoded bytes;
the committed fixture hashes below identify this generation.

The short Opus file fits within one Ogg page, intentionally exercising header
pages and retained audio packets after a target. `seek-long.opus` is a generated
2.4-second repeat/truncation of the source, with multiple audio pages and final
packet padding, exercising late seeks and an EOS-page anchor. The
codec test compares complete decoded tails with a linear decode. The playback
matrix compares retained-reader and fresh-cut PCM plus every frame's mapped time
across five formats, both MP3 timeline policies, three rates, two pitch settings
and two output sample rates, plus the long Opus case (84 configurations,
336 retained seeks).

```text
5fadf6550759fecd7bbc10b627b5968cf8a8352280896700b74649f797f351e9  seek-long.opus
f0c269e3080d2c080a61738266df5d27d25644982bfb392d87f3215b8fc1b182  seek.flac
c69312f9f8f265e1d7f24098e11b99efec46974bf230d6502f31be1b3974dbf0  seek.mp3
b32452501cdb0f7fc31c90368cd473e6fb630da057fc7c5465f41f9da3269f7e  seek.ogg
9aed54e966d577ffbcf51f1f6aa392b95387b32030a7a09f515f78181817b55a  seek.opus
d5a52be30706b6aafd8ff29294c71b62ab7346b53fc2d89f238fd4c9410a9820  seek.wav
```
