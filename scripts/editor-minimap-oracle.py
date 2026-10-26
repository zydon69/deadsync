#!/usr/bin/env python3
"""Extract unchanged pinned MiniMap math; no GUI, timing conversion or keyboard claim."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
METHODS = ["ReflectScreenYIfNeeded", "UpdateBasedOnSelectedPixel", "SetMiniMapAreaStart", "GetYPixelRelativeToBounds", "GetPositionRelativeToPixel"]

def extract(source, name):
    marker = next(line for line in source.splitlines() if line.startswith("\tprivate ") and f" {name}(" in line)
    start = source.index(marker)
    brace = source.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root", type=Path, required=True)
    parser.add_argument("--dotnet", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.reference_root.resolve()
    commit = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    if commit != PIN:
        raise SystemExit(f"Expected reference {PIN}, got {commit}")
    path = root / "StepManiaEditor/MiniMap.cs"
    source = path.read_text(encoding="utf-8-sig")
    methods = {name: extract(source, name) for name in METHODS}
    requests = []
    for reverse in [False, True]:
        for quantize in [False, True]:
            for full, mini, editor in [(1000., 400., 80.), (200., 400., 80.), (1000., 400., 400.)]:
                for pixel in [-120, 20, 100, 220, 320]:
                    requests.append(dict(FullStart=-10., FullEnd=full-10., MiniStart=-3.25,
                        MiniRange=mini, EditorStart=23.75, EditorEnd=23.75+editor,
                        BoundsY=20, VisibleHeight=200, GrabbedPercentage=0.375,
                        ScreenY=pixel, Reverse=reverse, Quantize=quantize))
    program = '''using System; using System.Text.Json;
class Input {
 public double FullStart,FullEnd,MiniStart,MiniRange,EditorStart,EditorEnd,GrabbedPercentage;
 public int BoundsY,VisibleHeight,ScreenY; public bool Reverse,Quantize;
}
class MiniMapReference {
 struct Rect { public int Y; }
 Rect Bounds; int VisibleHeight; bool Reverse,QuantizePositions;
 double FullAreaStart,FullAreaEnd,MiniMapAreaStart,MiniMapAreaStartUnquantized,MiniMapAreaRange,HeightOverMiniMapAreaRange;
 double EditorAreaStart,EditorAreaEnd,GrabbedPositionAsPercentageOfEditorArea;
 public object Run(Input i) {
  Bounds.Y=i.BoundsY; VisibleHeight=i.VisibleHeight; Reverse=i.Reverse; QuantizePositions=i.Quantize;
  FullAreaStart=i.FullStart; FullAreaEnd=i.FullEnd; MiniMapAreaRange=i.MiniRange;
  HeightOverMiniMapAreaRange=VisibleHeight/MiniMapAreaRange;
  EditorAreaStart=i.EditorStart; EditorAreaEnd=i.EditorEnd; GrabbedPositionAsPercentageOfEditorArea=i.GrabbedPercentage;
  SetMiniMapAreaStart(i.MiniStart);
  var reflected=ReflectScreenYIfNeeded(i.ScreenY);
  var position=GetPositionRelativeToPixel(reflected);
  var projected=GetYPixelRelativeToBounds(i.EditorStart,true);
  UpdateBasedOnSelectedPixel(reflected);
  return new {reflected,position,projected,miniStart=MiniMapAreaStart,miniUnquantized=MiniMapAreaStartUnquantized,editorStart=EditorAreaStart,editorEnd=EditorAreaEnd};
 }
''' + "\n".join(methods.values()) + '''
 static void Main() {
  var options=new JsonSerializerOptions {IncludeFields=true};
  var inputs=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),options);
  Console.Write(JsonSerializer.Serialize(Array.ConvertAll(inputs,i=>new MiniMapReference().Run(i))));
 }
}
'''
    with tempfile.TemporaryDirectory(prefix="deadsync-minimap-oracle-") as folder:
        project = Path(folder)
        (project / "oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (project / "Program.cs").write_text(program)
        subprocess.run([str(args.dotnet), "build", str(project / "oracle.csproj"), "--nologo", "-v", "q"], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        result = subprocess.run([str(args.dotnet), str(project / "bin/Debug/net10.0/oracle.dll")], input=json.dumps(requests), text=True, capture_output=True, check=True)
    expected = json.loads(result.stdout)
    document = dict(reference_commit=commit, source="StepManiaEditor/MiniMap.cs",
        source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        method_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name,body in methods.items()},
        scope="Unchanged projection/reflection/drag arithmetic only. Constructor fixture glue; no GUI hit testing, timing conversion, renderer or keyboard equivalence.",
        cases=[dict(input=request, expected=value) for request,value in zip(requests,expected)])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document,ensure_ascii=False,indent=2)+"\n")
    print(f"Generated {len(requests)} cases from {commit}")

if __name__ == "__main__":
    main()
