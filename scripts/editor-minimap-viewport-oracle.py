#!/usr/bin/env python3
"""Pinned unchanged C# viewport positioning/quantization; fixture geometry only."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-root",type=Path,required=True)
    parser.add_argument("--dotnet",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    spec=importlib.util.spec_from_file_location("minimap_reference",Path(__file__).with_name("editor-minimap-oracle.py"))
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
    commit=subprocess.check_output(["git","-C",str(args.reference_root),"rev-parse","HEAD"],text=True).strip()
    if commit!=helper.PIN: raise SystemExit("Wrong reference commit")
    path=args.reference_root/"StepManiaEditor/MiniMap.cs"
    source=path.read_text(encoding="utf-8-sig")
    methods={name:helper.extract(source,name) for name in ["SetMiniMapAreaFromEditorArea","SetMiniMapAreaStart"]}
    inputs=[]
    for start in [-100.,0.]:
        for full_range in [20.,1000.]:
            for editor_range in [10.,500.]:
                for mini_range in [20.,2000.]:
                    for editor in [-500.,0.,75.,1000.,2000.]:
                        for quantize in [False,True]:
                            inputs.append(dict(FullStart=start,FullEnd=start+full_range,EditorStart=editor,
                                EditorEnd=editor+editor_range,MiniRange=mini_range,VisibleHeight=240,Quantize=quantize))
    program='''using System;using System.Text.Json;
class Input {public double FullStart,FullEnd,EditorStart,EditorEnd,MiniRange;public int VisibleHeight;public bool Quantize;}
class Reference {
 double FullAreaStart,FullAreaEnd,EditorAreaStart,EditorAreaEnd,MiniMapAreaRange,MiniMapAreaStart,MiniMapAreaStartUnquantized;
 int VisibleHeight;bool QuantizePositions;
 public object Run(Input i) {
  FullAreaStart=i.FullStart;FullAreaEnd=i.FullEnd;EditorAreaStart=i.EditorStart;EditorAreaEnd=i.EditorEnd;
  MiniMapAreaRange=i.MiniRange;VisibleHeight=i.VisibleHeight;QuantizePositions=i.Quantize;
  SetMiniMapAreaFromEditorArea();
  return new {miniStart=MiniMapAreaStart,unquantized=MiniMapAreaStartUnquantized};
 }
'''+"\n".join(methods.values())+'''
 static void Main(){var input=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),new JsonSerializerOptions{IncludeFields=true});Console.Write(JsonSerializer.Serialize(Array.ConvertAll(input,i=>new Reference().Run(i))));}
}'''
    with tempfile.TemporaryDirectory(prefix="deadsync-minimap-viewport-") as folder:
        root=Path(folder)
        (root/"oracle.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (root/"Program.cs").write_text(program)
        subprocess.run([str(args.dotnet),"build",str(root/"oracle.csproj"),"--nologo","-v","q"],capture_output=True,check=True)
        result=subprocess.run([str(args.dotnet),str(root/"bin/Debug/net10.0/oracle.dll")],input=json.dumps(inputs),text=True,capture_output=True,check=True)
    expected=json.loads(result.stdout)
    output=dict(reference_commit=commit,source="StepManiaEditor/MiniMap.cs",source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        method_sha256={name:hashlib.sha256(body.encode()).hexdigest() for name,body in methods.items()},
        scope="Unchanged C# positioning and quantization only; fixture-supplied geometry, no native viewport, GUI, timing conversion or input claim.",
        cases=[dict(input=i,expected=e) for i,e in zip(inputs,expected)])
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(output,indent=2)+"\n")
    print(f"Generated {len(inputs)} cases from {commit}")
if __name__=="__main__":main()
