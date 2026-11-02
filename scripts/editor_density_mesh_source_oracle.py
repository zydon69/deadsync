#!/usr/bin/env python3
"""Execute the pinned original density measure-mesh body, real Fumen and MonoGame constructors.
External threading/GPU hosts are omitted. Geometry/colors are observed before GPU projection.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
import sys
sys.dont_write_bytecode=True
import editor_density_raw_source_oracle as source

def requests():
 result=[]
 for name,times,steps,rows,end in [
  ("nonbinary",[0,.1,.4,1.7,5.1],[3,7,11,13,17],[2,3,5,7,11],7.9),
  ("thirds",[0,1/3,2/3,1,4/3],[1,2,3,5,7],[1,1,2,3,4],5/3),
  ("gaps",[0,.7,1.1,2.3,4.9,5.2],[0,3,7,0,0,5],[0,2,5,0,0,3],7.1),
  ("near-rates",[0,1,2,3,4],[10,10,10,11,11],[5,5,5,7,7],5),
  ("zero-duration",[0,.1,.1,.7],[3,4,6,7],[2,2,3,5],1.1),
  ("negative-duration",[0,.7,.4,1.2],[3,4,6,7],[2,2,3,5],2.1)]:
  for accumulation in ["Step","Row"]:
   for mode in ["ColorByDensity","ColorByHeight"]:
    for width,height in [(37,23),(854,480),(2147483647,2147483647)]:
     result.append(dict(name=f"{name}-{accumulation}-{mode}-{width}",times=times,steps=steps,rows=rows,end=end,accumulation=accumulation,mode=mode,width=width,height=height))
 return result

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ["source","reference-root","dotnet","monogame-color","output"]:parser.add_argument("--"+name,type=Path,required=True)
 args=parser.parse_args();assert source.git(args.source,"rev-parse","HEAD").decode().strip()==source.EDITOR_PIN
 assert source.git(args.reference_root,"rev-parse","HEAD").decode().strip()==source.LIBRARY_PIN
 fumen=args.reference_root/"Fumen";assert source.git(fumen,"rev-parse","HEAD").decode().strip()==source.FUMEN_PIN
 dll=fumen/"Fumen/bin/Release/net10.0/Fumen.dll";assert dll.exists()
 effect=source.original(args.source,"StepManiaEditor/StepDensityEffect.cs");text=effect.decode("utf-8-sig")
 peak_decl=text[text.index("var peakNps = 0.0;"):text.index("// Early out on invalid bounds.")]
 start=text.index("// Determine the greatest number of steps per measure.")
 end=text.index("// Add the rim primitives.",text.index("// If we made it to the end"))
 body=peak_decl+text[start:end]
 instrumented_body=body.replace("var yPercent = stepsPerSecond / peakSteps;","var yPercent = stepsPerSecond / peakSteps; observedRates.Add(new {rate=stepsPerSecond,percent=yPercent});")
 measure=source.original(args.source,"StepManiaEditor/StepDensity.cs")
 measure_body=source.method(measure.decode("utf-8-sig"),"public readonly struct Measure")
 color=args.monogame_color.read_bytes();assert hashlib.sha256(color).hexdigest()==source.COLOR_SHA
 constructors="\n".join(source.method(color.decode("utf-8-sig"),s)for s in ["public Color(uint packedValue)","public Color(Vector4 color)","public Color(float r, float g, float b, float alpha)","public Color(int r, int g, int b, int alpha)"])
 color_utils=source.original(args.source,"MonoGameExtensions/ColorUtils.cs").decode("utf-8-sig")
 # This standalone compilation uses a component-preserving vector carrier and
 # value vertex. Source constructor, Fumen packed math and mesh body are unchanged.
 host="""using System;using System.Numerics;using System.Text.Json;using Fumen;using Color=Microsoft.Xna.Framework.Color;using ColorUtils=MonoGameExtensions.ColorUtils;
namespace Microsoft.Xna.Framework {public struct Color{uint _packedValue;public uint PackedValue=>_packedValue;public byte R=>(byte)_packedValue;public byte G=>(byte)(_packedValue>>8);public byte B=>(byte)(_packedValue>>16);CONSTRUCTORS}static class MathHelper{public static int Clamp(int v,int min,int max)=>Math.Clamp(v,min,max);}}
namespace StepManiaEditor {enum StepAccumulationType{Step,Row}enum DensityGraphColorMode{ColorByDensity,ColorByHeight}class StepDensity{MEASURE}struct VertexPositionColor{public Vector3 Position;public Color Color;public VertexPositionColor(Vector3 p,Color c){Position=p;Color=c;}}
class Program {
 static object Run(JsonElement input){var measures=new DynamicArray<StepDensity.Measure>(32);var times=input.GetProperty("times").EnumerateArray().Select(v=>v.GetDouble()).ToArray();var steps=input.GetProperty("steps").EnumerateArray().Select(v=>v.GetByte()).ToArray();var rows=input.GetProperty("rows").EnumerateArray().Select(v=>v.GetByte()).ToArray();for(int i=0;i<times.Length;i++)measures.Add(new(times[i],steps[i],rows[i]));
 double finalTime=input.GetProperty("end").GetDouble();float width=input.GetProperty("width").GetInt32(),height=input.GetProperty("height").GetInt32();const float RimW=1;
 var accumulationMode=Enum.Parse<StepAccumulationType>(input.GetProperty("accumulation").GetString());var colorMode=Enum.Parse<DensityGraphColorMode>(input.GetProperty("mode").GetString());
 var lowColor=new Color(.251f,.647f,.419f,1);var highColor=new Color(.705f,.282f,.282f,1);var vertices=new DynamicArray<VertexPositionColor>(32);var indices=new DynamicArray<int>(32);int numPrimitives=0;var observedRates=new List<object>();
 BODY
 var expanded=indices.GetArray().Take(indices.GetSize()).Select(index=>{var v=vertices[index];return new{x=v.Position.X,y=v.Position.Y,color=v.Color.PackedValue};}).ToArray();
 return new{input,peakNps,peakRps,numPrimitives,vertices=expanded,rates=observedRates};}
 static void Main(){Console.Write(JsonSerializer.Serialize(JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray().Select(Run).ToArray()));}
}}
""".replace("CONSTRUCTORS",constructors).replace("MEASURE",measure_body).replace("BODY",instrumented_body)
 with tempfile.TemporaryDirectory(prefix="deadsync-density-mesh-original-")as folder:
  work=Path(folder);(work/"Program.cs").write_text(host);(work/"ColorUtils.cs").write_text(color_utils)
  (work/"Oracle.csproj").write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
  result=subprocess.run([str(args.dotnet),"build",str(work/"Oracle.csproj"),"-c","Release","--nologo","-v","q","-p:WarningLevel=0"],capture_output=True,text=True)
  if result.returncode:raise SystemExit(result.stdout+result.stderr)
  assembly=work/"bin/Release/net10.0/Oracle.dll";result=subprocess.run([str(args.dotnet),str(assembly)],input=json.dumps(requests()),capture_output=True,text=True,check=True);expected=json.loads(result.stdout)
  provenance=dict(editor_commit=source.EDITOR_PIN,fumen_commit=source.FUMEN_PIN,monogame_commit=source.MONOGAME_PIN,effect_sha256=hashlib.sha256(effect).hexdigest(),step_density_sha256=hashlib.sha256(measure).hexdigest(),color_sha256=source.COLOR_SHA,original_body_sha256=hashlib.sha256(body.encode()).hexdigest(),host_sha256=hashlib.sha256(host.encode()).hexdigest(),cases=len(expected),scope=__doc__)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(expected,indent=2)+"\n");args.output.with_suffix(".reference.json").write_text(json.dumps(provenance,indent=2)+"\n");print("Executed",len(expected),"original density mesh bodies")
if __name__=="__main__":main()
