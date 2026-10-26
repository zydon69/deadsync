#!/usr/bin/env python3
"""Execute unchanged MiniMap helpers on isolated buffers and injected colors.
Does not run the original editor event enumeration, GPU renderer or palette.
"""
import argparse,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def extract(source,name):
 marker=next(line for line in source.splitlines() if line.startswith(("\tprivate ","\tpublic ")) and f" {name}(" in line)
 start=source.index(marker);brace=source.index('{',start);depth=1;end=brace+1
 while depth:
  depth+=(source[end]=='{')-(source[end]=='}');end+=1
 return source[start:end]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 spec=importlib.util.spec_from_file_location('helper',Path(__file__).with_name('editor-minimap-oracle.py'));h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
 names={'StepManiaEditor/MiniMap.cs':['UpdateLaneXPositions','AddShortNote','AddHorizontalLine','AddRect','AddHold','GetYPixelRelativeToBounds'],'StepManiaEditor/Utils.cs':['ColorRGBAInterpolateBGR']}
 bodies={};sources={}
 for path,methods in names.items():
  raw=(a.reference_root/path).read_bytes();pinned=subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{PIN}:{path}'])
  if raw!=pinned:raise SystemExit(f'Reference working tree differs from pin: {path}')
  text=raw.decode('utf-8-sig');sources[path]=hashlib.sha256(raw).hexdigest()
  bodies.update({name:extract(text,name) for name in methods})
 cases=[]
 for width,lanes in [(16,4),(9,8),(32,1)]:
  for quantize in [False,True]:
   for kind in ['short','cursor','rect','hold']:
    for start,end in [(-5.,-2.),(-3.25,4.5),(0.,1.),(1.,1.75),(1.25,8.75),(14.75,20.),(15.,20.)]:
     cases.append(dict(width=width,height=16,lanes=lanes,noteWidth=2,noteSpacing=3,quantize=quantize,kind=kind,start=start,end=end,lane=lanes-1))
 program='''using System;using System.Text.Json;
class Input {public int width,height;public uint lanes,noteWidth,noteSpacing,lane;public bool quantize;public string kind;public double start,end;}
class BoundsData {public int Width;}
class Buffer {public uint[] Pixels;public uint[] GetCurrentData()=>Pixels;}
class EditorHoldNoteEvent {public int Lane;public int GetLane()=>Lane;public int GetStepColorRow()=>0;public int GetPlayer()=>0;}
class Palette {public uint GetRollUIColor(int row,int lane,bool selected,int player)=>0xff3355ddu;public uint GetHoldUIColor(int row,int lane,bool selected,int player)=>0xff3355ddu;public uint GetArrowUIColor(int row,int lane,bool selected,int player)=>0xffef4477u;}
static class MathUtils {public static int FloorDouble(double value)=>(int)Math.Floor(value);}
static class Utils {'''+bodies['ColorRGBAInterpolateBGR']+'''}
class Reference {
 public enum AddResult {AboveTop,InRange,BelowBottom}
 const int RimWidth=1;BoundsData Bounds=new();int VisibleHeight;uint NoteWidth,NoteSpacing,NumLanes;uint[] LaneXPositions;Buffer RenderTarget=new();bool QuantizePositions;double MiniMapAreaStart,MiniMapAreaStartUnquantized,HeightOverMiniMapAreaRange=1.0;Palette ArrowGraphicManager=new();
 public object Run(Input i) {
  Bounds.Width=i.width;VisibleHeight=i.height;NoteWidth=i.noteWidth;NoteSpacing=i.noteSpacing;NumLanes=i.lanes;QuantizePositions=i.quantize;
  RenderTarget.Pixels=new uint[i.width*i.height];Array.Fill(RenderTarget.Pixels,0xff102030u);UpdateLaneXPositions();
  AddResult result;
  if(i.kind=="short")result=AddShortNote(GetYPixelRelativeToBounds(i.start),LaneXPositions[i.lane],0xffef4477u);
  else if(i.kind=="cursor")result=AddHorizontalLine(GetYPixelRelativeToBounds(i.start,true),1,(uint)i.width-2,0xffef4477u,true);
  else if(i.kind=="rect")result=AddRect(1,(uint)i.width-2,GetYPixelRelativeToBounds(i.start),GetYPixelRelativeToBounds(i.end),0xffef4477u);
  else result=AddHold(new EditorHoldNoteEvent{Lane=(int)i.lane},i.start,i.end,false,false);
  return new {result=result.ToString(),lanes=LaneXPositions,pixels=RenderTarget.Pixels};
 }
'''+ '\n'.join(body for name,body in bodies.items() if name!='ColorRGBAInterpolateBGR')+'''
 static void Main(){var options=new JsonSerializerOptions{IncludeFields=true};var inputs=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),options);Console.Write(JsonSerializer.Serialize(Array.ConvertAll(inputs,i=>new Reference().Run(i))));}
}'''
 with tempfile.TemporaryDirectory(prefix='deadsync-minimap-content-') as folder:
  root=Path(folder);(root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
  build=subprocess.run([str(a.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if build.returncode:raise SystemExit(build.stdout+build.stderr)
  run=subprocess.run([str(a.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),capture_output=True,text=True,check=True)
 expected=json.loads(run.stdout)
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(dict(reference_commit=PIN,source_sha256=sources,method_sha256={name:hashlib.sha256(body.encode()).hexdigest() for name,body in bodies.items()},scope='Unchanged C# helpers; fixed palette and isolated pixel buffer adapters; no event-enumeration/native-palette/GPU parity claim.',cases=[dict(input=i,expected=e) for i,e in zip(cases,expected)]),indent=2)+'\n')
 print(f'Generated {len(cases)} raster cases from pinned source bytes')
if __name__=='__main__':main()
