#!/usr/bin/env python3
"""Run the unchanged pinned GrooveAuthor asset selector with injected dimensions.

The Skia codec is replaced by fixture dimensions: this verifies selection rules,
not real image decoding or native dialogs. Path conversion uses .NET's lexical
GetRelativePath on same-root absolute fixture paths.
"""
import argparse
import hashlib
import json
from pathlib import Path
import random
import subprocess
import tempfile

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--reference-root", type=Path, required=True)
parser.add_argument("--dotnet", type=Path, required=True)
parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "crates/deadsync-editor/tests/fixtures/grooveauthor/song-assets-discovery.json")
args = parser.parse_args()
actual = subprocess.check_output(["git", "-C", str(args.reference_root), "rev-parse", "HEAD"], text=True).strip()
if actual != PIN:
    raise SystemExit(f"Expected GrooveAuthor {PIN}, found {actual}")
source = args.reference_root / "StepManiaEditor/EditorSongImageUtils.cs"
original = source.read_bytes()
# Verify the tracked source too: matching HEAD alone does not prove file fidelity.
tracked = subprocess.check_output(["git", "-C", str(args.reference_root), "show", f"{PIN}:StepManiaEditor/EditorSongImageUtils.cs"])
if original != tracked:
    raise SystemExit("Reference selector has uncommitted changes")

def file(name, w=None, h=None):
    return {"Name": name, "Width": w, "Height": h}

cases = [
    {"Files": []},
    {"Files": [file("background-banner.png"), file("é.lrc"), file("two.lrc")]},
    {"Files": [file("background.PNG",640,480),file("banner.JPG",418,164),file("x.LRC")]},
    {"Files": [file("first.png",320,240),file("second.png",640,480)]},
    {"Files": [file("small.png",32,24),file("big.png",1280,960)]},
    {"Files": [file("movie.mp4"),file("wide.png",1000,600),file("bigger.png",2000,1200)]},
    {"Files": [file("tiny.png",639,400),file("clip.webm")]},
    {"Files": [file("first.png",1000,600),file("tie.png",1200,500)]},
    {"Files": [file("ratio.png",209,82),file("exact.png",418,164)]},
    {"Files": [file("x.png",2_000_000_000,2_000_000_000),file("movie.mp4")]},
    {"Files": [file("background.mp4"),file("banner.mp4"),file("title.avi")]},
    {"Files": [file("jk_cover.png",500,500),file("x-cd.png"),file("song disc.png"),file("cdtitle.gif"),file("x bn.jpg")]},
    {"Files": [file("jacket.png",640,480),file("jacket2.png",418,164),file("unknown.png",500,500)]},
    {"Files": [file("zero.png",0,480),file("negative.png",-640,480)]},
]
rng = random.Random(460815)
names = ["a.png","b.jpg","background.png","xbg.webp","banner.png","x bn.gif","cdtitle.tif","jk_cover.png","albumart.bmp","x-cd.png","x disc.png","x title.png","clip.mp4","video.avi","ignored.PNG","words.lrc","ignored.LRC"]
sizes = [(None,None),(640,480),(418,164),(320,240),(209,82),(1000,600),(1200,500),(256,256),(640,481),(640,500),(0,0),(2_000_000_000,2_000_000_000)]
for _ in range(100):
    chosen = rng.sample(names,rng.randint(1,len(names)))
    cases.append({"Files": [file(name,*rng.choice(sizes)) for name in chosen]})

stubs = r'''
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
namespace StepManiaEditor {
internal static class Utils {
 public static readonly string[] ExpectedImageFormats = ["bmp","gif","jpeg","jpg","png","tif","tiff","webp"];
 public static readonly string[] ExpectedVideoFormats = ["avi","f4v","flv","mkv","mp4","mpeg","mpg","mov","ogv","webm","wmv"];
 public static readonly string[] ExpectedLyricsFormats = ["lrc"];
}
internal static class ImGuiUtils {
 public const int BackgroundWidthDefaultDPI=640,BackgroundHeightDefaultDPI=480,BannerWidthDefaultDPI=418,BannerHeightDefaultDPI=164;
}
}
namespace Fumen { internal static class Path { public static string GetRelativePath(string directory,string path) => System.IO.Path.GetRelativePath(directory,path); } }
namespace SkiaSharp {
 internal sealed class SKCodec : IDisposable {
  internal static Dictionary<string,(int,int)> Dimensions = new();
  public (int Width,int Height) Info {get;private set;}
  public static SKCodec Create(string path) => new SKCodec {Info=Dimensions[path]};
  public void Dispose() {}
 }
}
internal static class Program {
 public static void Main() {
  var requests=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;
  var results=new List<object>();
  foreach(var request in requests.EnumerateArray()) {
   var files=request.GetProperty("Files").EnumerateArray().ToArray();
   string Full(JsonElement file)=>"/songs/example/"+file.GetProperty("Name").GetString();
   SkiaSharp.SKCodec.Dimensions.Clear();
   foreach(var file in files) if(file.GetProperty("Width").ValueKind==JsonValueKind.Number) SkiaSharp.SKCodec.Dimensions[Full(file)]=(file.GetProperty("Width").GetInt32(),file.GetProperty("Height").GetInt32());
   var paths=files.Select(Full).ToArray();
   var images=StepManiaEditor.EditorSongImageUtils.TryFindBestImages("/songs/example",paths).ToDictionary(pair=>pair.Key.ToString(),pair=>pair.Value);
   var lyrics=StepManiaEditor.EditorSongImageUtils.TryFindBestLyrics("/songs/example",paths);
   var types=paths.Select(path=>StepManiaEditor.EditorSongImageUtils.GetBestSongImageType(path).ToString()).ToArray();
   results.Add(new {Images=images,Lyrics=lyrics,Types=types});
  }
  Console.Write(JsonSerializer.Serialize(results));
 }
}
'''
with tempfile.TemporaryDirectory(prefix="deadsync-assets-oracle-") as temp:
    project = Path(temp)
    (project / "Original.cs").write_bytes(original)
    (project / "Program.cs").write_text(stubs)
    (project / "AssetsOracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><CheckForOverflowUnderflow>false</CheckForOverflowUnderflow></PropertyGroup></Project>')
    subprocess.run([str(args.dotnet), "build", str(project), "--configuration", "Release", "--nologo", "--verbosity", "quiet"],check=True)
    output=subprocess.check_output([str(args.dotnet),str(project / "bin/Release/net10.0/AssetsOracle.dll")],input=json.dumps(cases).encode())
results=json.loads(output)
assert len(results)==len(cases)
payload={"Reference":PIN,"SourceSha256":hashlib.sha256(original).hexdigest(),"AdapterSha256":hashlib.sha256(stubs.encode()).hexdigest(),"Scope":"Unchanged EditorSongImageUtils selection and classification; injected codec dimensions; lexical .NET same-root paths. No real Skia decoding, filesystem enumeration order, native UI or file writes proven.","Cases":[dict(case,Expected=result) for case,result in zip(cases,results)]}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(payload,ensure_ascii=False,indent=2)+"\n")
print(f"Wrote {len(cases)} source-derived asset discovery cases to {args.output}")
