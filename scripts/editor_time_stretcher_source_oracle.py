#!/usr/bin/env python3
"""Execute GrooveAuthor's unmodified TimeStretcher against pinned real Fumen.

This proves interpolation at the float PCM boundary, not codec decoding or the
native transport. All resulting samples are recorded as exact IEEE float bits.
"""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

EDITOR_PIN = '48e18f71655d0f0d86144c63239331833501fda2'
FUMEN_PIN = '0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['editor-root', 'reference-root', 'dotnet', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--bits-output', type=Path)
    args = parser.parse_args()
    fumen = args.reference_root / 'Fumen'
    for root, pin in [(args.editor_root, EDITOR_PIN), (fumen, FUMEN_PIN)]:
        actual = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
        if actual != pin:
            raise RuntimeError(f'Unexpected source revision: {root}: {actual}')
    source = subprocess.check_output(['git', '-C', str(args.editor_root), 'show',
        EDITOR_PIN + ':StepManiaEditor/Sound/TimeStretcher.cs'])
    dll = fumen / 'Fumen/bin/Release/net10.0/Fumen.dll'
    harness = '''using System.Text.Json;using StepManiaEditor;
var cases=new List<object>();
foreach(var hz in new uint[]{24000,44100,48000,96000})
foreach(var channels in new[]{1,2,3})
foreach(var frames in new[]{1,7,31})
foreach(var start in new long[]{-3,0,1,6,30,32})
foreach(var rate in new[]{0.25,0.3,0.9,1.0,1.1,1.35,2.0,4.0}){
 var input=new float[frames*channels];
 for(int i=0;i<input.Length;i++)input[i]=((i*37)%113-56)/64.0f;
 var output=new float[17*channels];
 TimeStretcher.ProcessSound(start,input,hz,channels,output,rate);
 cases.Add(new{hz,channels,frames,start,rate,
  inputBits=input.Select(BitConverter.SingleToInt32Bits).ToArray(),
  outputBits=output.Select(BitConverter.SingleToInt32Bits).ToArray()});
}
Console.Write(JsonSerializer.Serialize(cases));
'''
    with tempfile.TemporaryDirectory(prefix='deadsync-time-stretcher-') as directory:
        work = Path(directory)
        (work / 'TimeStretcher.cs').write_bytes(source)
        (work / 'Program.cs').write_text(harness)
        (work / 'Oracle.csproj').write_text(
            '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType>'
            '<TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings>'
            '</PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'
            + str(dll) + '</HintPath></Reference></ItemGroup></Project>')
        build = subprocess.run([str(args.dotnet), 'build', str(work / 'Oracle.csproj'),
            '-c', 'Release', '--nologo', '--verbosity', 'quiet'], capture_output=True, text=True)
        if build.returncode:
            raise RuntimeError(build.stdout + build.stderr)
        output = subprocess.check_output([str(args.dotnet), str(work / 'bin/Release/net10.0/Oracle.dll')])
    cases = json.loads(output)
    assert len(cases) == 1728
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(cases, separators=(',', ':')) + '\n')
    bits_sha = None
    if args.bits_output:
        args.bits_output.parent.mkdir(parents=True, exist_ok=True)
        args.bits_output.write_text(''.join(';'.join([str(c['hz']), str(c['channels']), str(c['start']), repr(c['rate']), ','.join(str(v & 0xffffffff) for v in c['inputBits']), ','.join(str(v & 0xffffffff) for v in c['outputBits'])]) + '\n' for c in cases))
        bits_sha = hashlib.sha256(args.bits_output.read_bytes()).hexdigest()
    args.output.with_suffix('.reference.json').write_text(json.dumps({
        'editor_pin': EDITOR_PIN, 'fumen_pin': FUMEN_PIN,
        'source_sha256': hashlib.sha256(source).hexdigest(),
        'fumen_dll_sha256': hashlib.sha256(dll.read_bytes()).hexdigest(),
        'harness_sha256': hashlib.sha256(harness.encode()).hexdigest(),
        'fixture_sha256': hashlib.sha256(args.output.read_bytes()).hexdigest(),
        'derived_bits_sha256': bits_sha,
        'cases': len(cases), 'boundary': 'Original ProcessSound and real Fumen Hermite; float PCM only',
        'limits': ['No codec decoding, callback scheduling, native transport, or source rate=1 bypass execution']
    }, indent=2) + '\n')
    print(f'Executed original TimeStretcher: {len(cases)} exact float PCM cases')


if __name__ == '__main__':
    main()
