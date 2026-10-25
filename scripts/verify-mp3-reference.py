#!/usr/bin/env python3
"""Verify the test-only MP3 oracle; --git also checks its recorded Git source."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--git', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
reference = root / 'crates/deadlib-audio-decode/tests/reference/mp3_before_timeline_options.rs'
manifest = json.loads(reference.with_suffix('.json').read_text())
source = reference.read_bytes()
if hashlib.sha256(source).hexdigest() != manifest['production_source_sha256']:
    raise SystemExit('MP3 reference SHA-256 mismatch')
hash_value = 0xcbf29ce484222325
for byte in source:
    hash_value = ((hash_value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
if f'0x{hash_value:016x}' != manifest['production_fnv1a64']:
    raise SystemExit('MP3 reference source fingerprint mismatch')
def helper_body(source, name):
    start = source.index('fn ' + name + '(')
    end = source.index('{', start) + 1
    depth = 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]

for helper in manifest['shared_helpers']:
    body = helper_body((root / helper['path']).read_text(), helper['name'])
    if hashlib.sha256(body.encode()).hexdigest() != helper['body_sha256']:
        raise SystemExit('MP3 shared helper changed: ' + helper['name'])
    if args.git:
        original_helper = subprocess.check_output(['git', 'show', manifest['commit'] + ':' + helper['path']], cwd=root, text=True)
        if body != helper_body(original_helper, helper['name']):
            raise SystemExit('MP3 helper differs from original: ' + helper['name'])

if args.git:
    original = subprocess.check_output(['git', 'show', manifest['commit'] + ':' + manifest['path']], cwd=root)
    if hashlib.sha256(original).hexdigest() != manifest['full_source_sha256']:
        raise SystemExit('Original Git source SHA-256 mismatch')
    production = original.split(b'\n#[cfg(test)]\nmod tests {', 1)[0] + b'\n'
    if production != source:
        raise SystemExit('MP3 reference differs from original production source')
print('MP3 original-source oracle verified' + (' against Git' if args.git else ''))
