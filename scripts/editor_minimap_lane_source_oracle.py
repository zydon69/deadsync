#!/usr/bin/env python3
"""Execute the pinned original MiniMap lane method using .NET, without Cargo."""
import argparse
import pathlib
import subprocess
import tempfile

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
parser = argparse.ArgumentParser()
parser.add_argument("--source", required=True, type=pathlib.Path)
parser.add_argument("--dotnet", required=True, type=pathlib.Path)
parser.add_argument("--output", required=True, type=pathlib.Path)
args = parser.parse_args()
source = subprocess.check_output(["git", "-C", str(args.source), "show", PIN + ":StepManiaEditor/MiniMap.cs"], text=True)
a = source.index("\tprivate void UpdateLaneXPositions()")
b = source.index("\n\t/// <summary>", a)
method = source[a:b]
with tempfile.TemporaryDirectory(prefix="deadsync-minimap-source-") as directory:
    project = pathlib.Path(directory)
    (project / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
    (project / "Program.cs").write_text('''using System.Text.Json;
var rows=new List<object>();
foreach(var width in new[]{2,3,8,128})foreach(uint nw in new uint[]{1,2,32})foreach(uint spacing in new uint[]{0,32}) {var m=new MiniMap(width,4,nw,spacing);rows.Add(new{width,note_width=nw,note_spacing=spacing,positions=m.Get()});}
Console.WriteLine(JsonSerializer.Serialize(rows));
class MiniMap {
 const int RimWidth=1; uint NumLanes, NoteWidth,NoteSpacing; uint[] LaneXPositions; Rect Bounds;
 record Rect(int Width);
 public MiniMap(int width,uint lanes,uint nw,uint ns){Bounds=new(width);NumLanes=lanes;NoteWidth=nw;NoteSpacing=ns;}
 public uint[] Get(){UpdateLaneXPositions();return LaneXPositions;}
''' + method + "\n}")
    output = subprocess.check_output([str(args.dotnet), "run", "--project", str(project / "oracle.csproj"), "--configuration", "Release"], text=True)
    # Preserve only the JSON result; SDK first-use notices are not fixture data.
    import json
    rows = json.loads(next(line for line in reversed(output.splitlines()) if line.startswith("[")))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(rows, indent=2) + "\n")
