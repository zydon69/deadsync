#!/usr/bin/env python3
"""Run the pinned original SoundMipMap task planner on .NET; write Rust tuples."""
import argparse
import json
import pathlib
import subprocess
import tempfile
PIN = "48e18f71655d0f0d86144c63239331833501fda2"
p = argparse.ArgumentParser()
p.add_argument("--source", required=True, type=pathlib.Path)
p.add_argument("--dotnet", required=True, type=pathlib.Path)
p.add_argument("--output", required=True, type=pathlib.Path)
a = p.parse_args()
source = subprocess.check_output(["git", "-C", str(a.source), "show", PIN + ":StepManiaEditor/Sound/SoundMipMap.cs"], text=True)
start = source.index("\t\t\t// Determine the number of mip levels needed.")
end = source.index("\t\t\t// Divide up the mip map generation", start)
body = source[start:end]
with tempfile.TemporaryDirectory(prefix="deadsync-waveform-source-") as directory:
    root = pathlib.Path(directory)
    (root / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
    (root / "Program.cs").write_text('''using System.Text.Json;
var rows=new List<object>();foreach(uint frames in new uint[]{3,257,4097,10003})foreach(int load in new[]{-7,0,1,2,8,128,513,int.MaxValue}){var result=Select(frames,load);rows.Add(new{frames,parallelism=load,span=result.Item1,tasks=result.Item2});}Console.WriteLine(JsonSerializer.Serialize(rows));
static (uint,uint) Select(uint totalNumSamples,int LoadParallelism){int numChannels=2;object MipLevelsLock=new();MipLevel[] MipLevels;bool MipLevelsAllocated=false;var token=CancellationToken.None;
''' + body + "return(samplesPerTask,numTasks); }\nclass MipLevel{public MipLevel(uint samples,uint channels){}}")
    result = subprocess.check_output([str(a.dotnet), "run", "--project", str(root / "oracle.csproj"), "--configuration", "Release"], text=True)
    rows = json.loads(next(line for line in reversed(result.splitlines()) if line.startswith("[")))
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text("&[\n" + "".join(f"    ({r['frames']}, {r['parallelism']}, {r['span']}, {r['tasks']}),\n" for r in rows) + "]\n")
