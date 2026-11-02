#!/usr/bin/env python3
"""Original Fumen/FM0D decode witness, macOS NOSOUND_NRT (no physical output).
Requires the pinned Fumen build and .NET; outputs original format/frame/PCM hashes.
No editor or gameplay transport acknowledgements are fabricated.
"""
import argparse,hashlib,json,pathlib,subprocess,tempfile
from xml.sax.saxutils import escape
PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
PROGRAM='using System;using System.Runtime.InteropServices;using System.Threading;using System.Text.Json;\nclass P{\n static void Ok(FMOD.RESULT result){if(result!=FMOD.RESULT.OK)throw new Exception(result.ToString());}\n static ulong Hash(float[] data){ulong h=1469598103934665603;foreach(var x in data)h=unchecked((h^(uint)BitConverter.SingleToInt32Bits(x))*1099511628211);return h;}\n static void Main(string[] paths){NativeLibrary.SetDllImportResolver(typeof(FMOD.Factory).Assembly,(name,assembly,search)=>NativeLibrary.Load("__FMOD_NATIVE_LIBRARY__"));Ok(FMOD.Factory.System_Create(out var system));try{Ok(system.setOutput(FMOD.OUTPUTTYPE.NOSOUND_NRT));Ok(system.init(100,FMOD.INITFLAGS.NORMAL,IntPtr.Zero));foreach(var path in paths){if(path.EndsWith(".opus",StringComparison.OrdinalIgnoreCase)){var decoded=Fumen.SoundManager.DecodeOpusAsync(path,48000,CancellationToken.None).GetAwaiter().GetResult();Console.WriteLine(JsonSerializer.Serialize(new{path,route="original Fumen DecodeOpusAsync",channels=decoded.NumChannels,outputFrames=decoded.Samples.Length/decoded.NumChannels,hash=Hash(decoded.Samples)}));continue;}Ok(system.createSound(path,FMOD.MODE.DEFAULT,out var sound));try{Ok(sound.getFormat(out var type,out var format,out var channels,out var bits));Ok(sound.getDefaults(out var hz,out _));Ok(sound.getLength(out var frames,FMOD.TIMEUNIT.PCM));if(!Fumen.SoundManager.AllocateSampleBuffer(sound,48000,out var data,out var sampleChannels))throw new Exception("AllocateSampleBuffer");if(!Fumen.SoundManager.FillSamplesAsync(sound,48000,data,sampleChannels,CancellationToken.None).GetAwaiter().GetResult())throw new Exception("FillSamplesAsync");Console.WriteLine(JsonSerializer.Serialize(new{path,type=type.ToString(),format=format.ToString(),channels,bits,hz,frames,outputFrames=data.Length/sampleChannels,hash=Hash(data)}));}finally{Ok(sound.release());}}}finally{system.close();system.release();}}\n}\n'
def rewrite_opus_metadata(source,target):
 data=bytearray(source.read_bytes());offset=0;headers=0
 while offset<len(data):
  if data[offset:offset+4]!=b"OggS" or offset+27>len(data):raise ValueError("Invalid witness Ogg page")
  segments=data[offset+26];payload=offset+27+segments
  if payload>len(data):raise ValueError("Incomplete Ogg lacing")
  size=27+segments+sum(data[offset+27:payload])
  if offset+size>len(data):raise ValueError("Incomplete Ogg payload")
  if data[payload:payload+8]==b"OpusHead":
   headers+=1;data[payload+10:payload+12]=(0).to_bytes(2,"little");data[payload+16:payload+18]=(1536).to_bytes(2,"little",signed=True)
  if data[offset+5]&4:data[offset+6:offset+14]=(0).to_bytes(8,"little")
  data[offset+22:offset+26]=bytes(4);crc=0
  for byte in data[offset:offset+size]:
   crc^=byte<<24
   for _ in range(8):crc=((crc<<1)^0x04c11db7 if crc&0x80000000 else crc<<1)&0xffffffff
  data[offset+22:offset+26]=crc.to_bytes(4,"little");offset+=size
 if headers!=1:raise ValueError("Witness requires one Opus logical stream")
 target.write_bytes(data)

def main():
 p=argparse.ArgumentParser();p.add_argument("--fumen",type=pathlib.Path,required=True);p.add_argument("--dotnet",required=True);p.add_argument("--concentus",type=pathlib.Path,required=True);p.add_argument("--opus-metadata-witness",action="store_true");p.add_argument("assets",nargs="+");a=p.parse_args()
 head=subprocess.check_output(["git","-C",str(a.fumen),"rev-parse","HEAD"],text=True).strip()
 if head!=PIN:raise SystemExit("Fumen revision mismatch: "+head)
 dll=a.fumen/"Fumen/bin/Release/net10.0/Fumen.dll";native=a.fumen/"Fumen/Sound/fmod/lib/mac/libfmod.dylib"
 if not dll.is_file() or not native.is_file():raise SystemExit("Build pinned Fumen Release/net10.0 first; macOS original FMOD library required")
 subprocess.run(["git","-C",str(a.fumen),"diff","--exit-code",PIN,"--","Fumen/Sound/SoundManager.cs","Fumen/Sound/fmod"],check=True)
 print(json.dumps({"provenance":{"fumen_revision":head,"fumen_dll_sha256":hashlib.sha256(dll.read_bytes()).hexdigest(),"fmod_dylib_sha256":hashlib.sha256(native.read_bytes()).hexdigest(),"route":"original MODE.DEFAULT/NOSOUND_NRT FillSamples; original DecodeOpusAsync for .opus"}}),flush=True)
 with tempfile.TemporaryDirectory(prefix="source-fmod-pcm-")as temp:
  root=pathlib.Path(temp);program=PROGRAM.replace("__FMOD_NATIVE_LIBRARY__",str(native.resolve()))
  (root/"Program.cs").write_text(program)
  (root/"O.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><AllowUnsafeBlocks>true</AllowUnsafeBlocks><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+escape(str(dll.resolve()))+'</HintPath></Reference><Reference Include="Concentus"><HintPath>'+escape(str(a.concentus.resolve()))+'</HintPath></Reference></ItemGroup></Project>')
  subprocess.run([a.dotnet,"build",str(root/"O.csproj"),"--nologo","-v:q"],check=True)
  assets=[pathlib.Path(asset).resolve()for asset in a.assets]
  if a.opus_metadata_witness:
   original=next((asset for asset in assets if asset.suffix.lower()==".opus"),None)
   if original is None:raise SystemExit("Opus metadata witness requires a .opus asset")
   modified=root/"metadata-modified.opus";rewrite_opus_metadata(original,modified);assets.append(modified)
  output=subprocess.check_output([a.dotnet,str(root/"bin/Debug/net10.0/O.dll"),*[str(asset)for asset in assets]],text=True);print(output,end="")
  if a.opus_metadata_witness:
   rows=[json.loads(line)for line in output.splitlines()if line.startswith("{")];before=next(row for row in rows if row["path"]==str(original));after=next(row for row in rows if row["path"]==str(modified))
   for key in ("channels","outputFrames","hash"):
    if before[key]!=after[key]:raise AssertionError("Original Opus metadata witness differs: "+key)
   print(json.dumps({"opus_metadata_invariance":"PASS","mutations":["gain+6dB","pre_skip=0","final_granule=0"],"outputFrames":before["outputFrames"],"hash":before["hash"]}))
if __name__=="__main__":main()
