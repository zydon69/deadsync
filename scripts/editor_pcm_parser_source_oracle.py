#!/usr/bin/env python3
"""Execute unchanged pinned Fumen PCM parser with explicit raw byte inputs."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--reference-root',type=Path,required=True)
p.add_argument('--dotnet',type=Path,required=True)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
pin='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
root=a.reference_root/'Fumen'
assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
path='Fumen/Sound/SoundManager.cs'
raw=subprocess.check_output(['git','-C',str(root),'show',pin+':'+path])
s=raw.decode('utf-8-sig')
start=s.index('public static unsafe Func<long, float> GetParseSampleFunc(')
end=s.index('{',start)+1
depth=1
while depth:
 depth+=(s[end]=='{')-(s[end]=='}');end+=1
method=s[start:end]
program='''using System;using System.Collections.Generic;using System.Text.Json;
enum SOUND_FORMAT{PCM8,PCM16,PCM24,PCM32,PCMFLOAT}
static class Program{
METHOD
static unsafe void Main(){var result=new List<object>();
var cases=new (SOUND_FORMAT Format,byte[] Bytes)[]{
(SOUND_FORMAT.PCM8,new byte[]{0,1,127,128,254,255}),
(SOUND_FORMAT.PCM16,Convert.FromHexString("00000100FF7F0080FFFFD2042EFB")),
(SOUND_FORMAT.PCM24,Convert.FromHexString("000000010000FFFF7F000080FFFFFF123456EFCDAB")),
(SOUND_FORMAT.PCM32,Convert.FromHexString("0000000001000000FFFFFF7F00000080FFFFFFFF12345678EFCDAB89")),
(SOUND_FORMAT.PCMFLOAT,Convert.FromHexString("00000000000000800000803F000080BF0000003F0000807F000080FF0100C07F"))};
foreach(var c in cases){int width=c.Format switch{SOUND_FORMAT.PCM8=>1,SOUND_FORMAT.PCM16=>2,SOUND_FORMAT.PCM24=>3,_=>4};var bits=new List<string>();fixed(byte* ptr=c.Bytes){var parse=GetParseSampleFunc(c.Format,ptr);for(int i=0;i<c.Bytes.Length;i+=width)bits.Add(BitConverter.SingleToUInt32Bits(parse(i)).ToString("x8"));}result.Add(new{Format=c.Format.ToString(),RawHex=Convert.ToHexString(c.Bytes),SampleBits=bits});}
Console.Write(JsonSerializer.Serialize(result,new JsonSerializerOptions{WriteIndented=true}));}}
'''.replace('METHOD',method)
with tempfile.TemporaryDirectory(prefix='deadsync-original-pcm-parser-') as folder:
 work=Path(folder)
 (work/'Program.cs').write_text(program)
 (work/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><AllowUnsafeBlocks>true</AllowUnsafeBlocks></PropertyGroup></Project>')
 r=subprocess.run([str(a.dotnet),'build',str(work/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 if r.returncode:raise RuntimeError(r.stdout)
 result=json.loads(subprocess.check_output([str(a.dotnet),str(work/'bin/Release/net10.0/Oracle.dll')]))
assert len(result)==5 and sum(len(r['SampleBits']) for r in result)==35
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps(result,indent=2)+'\n')
a.output.with_suffix('.provenance.json').write_text(json.dumps({'FumenCommit':pin,'SourcePath':path,'SourceSha256':hashlib.sha256(raw).hexdigest(),'MethodSha256':hashlib.sha256(method.encode()).hexdigest(),'Scope':'Unmodified GetParseSampleFunc with minimal SOUND_FORMAT enum names and explicit raw byte input. No FMOD container decode, resampling, channel mapping or playback is executed. PCM8 follows the unsigned original contract; float nonfinite payload bits are observed, not certified safe for downstream playback.','Samples':35},indent=2)+'\n')
print('PASS: 35 original parser sample bit patterns across PCM8/16/24/32/FLOAT.')
