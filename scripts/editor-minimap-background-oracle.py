#!/usr/bin/env python3
"""Unchanged UpdateBegin/UpdateEnd on a buffer; marker calls recorded separately."""
import argparse,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 spec=importlib.util.spec_from_file_location('helpers',Path(__file__).with_name('editor-minimap-content-oracle.py'));h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
 source_path=a.reference_root/'StepManiaEditor/MiniMap.cs';raw=source_path.read_bytes()
 if raw!=subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{PIN}:StepManiaEditor/MiniMap.cs']):raise SystemExit('Wrong pinned source bytes')
 source=raw.decode('utf-8-sig');methods={name:h.extract(source,name) for name in ['UpdateBegin','UpdateEnd','SetMiniMapAreaFromEditorArea','SetMiniMapAreaStart','GetYPixelRelativeToBounds','GetEditorAreaPercentageOfMiniMapArea']}
 utils=(a.reference_root/'StepManiaEditor/Utils.cs').read_bytes()
 if utils!=subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{PIN}:StepManiaEditor/Utils.cs']):raise SystemExit('Wrong Utils source bytes')
 methods['ColorRGBAInterpolateBGR']=h.extract(utils.decode('utf-8-sig'),'ColorRGBAInterpolateBGR')
 cases=[]
 for start in [-20.0,0.0,3.25,15.0]:
  for size in [2.5,14.4,15.2,16.0]:
   for quantize in [False,True]:
    for state in ['normal','hover','grabbed']:cases.append(dict(editorStart=start,editorEnd=start+size,quantize=quantize,state=state))
 program='''using System;using System.Collections.Generic;using System.Text.Json;
class Input {public double editorStart,editorEnd;public bool quantize;public string state;}
class BoundsData {public int Width=16,Height=16;}
class Buffer {public uint[] Data=new uint[256];public uint[] GetCurrentData()=>Data;}
class ArrowGraphicManager {}
static class MathUtils {public static int FloorDouble(double value)=>(int)Math.Floor(value);}
static class Utils {'''+methods['ColorRGBAInterpolateBGR']+'''}
static class Interpolation { public static double Lerp(double startValue,double endValue,double startTime,double endTime,double currentTime) {if(Math.Abs(BitConverter.DoubleToInt64Bits(endTime)-BitConverter.DoubleToInt64Bits(startTime))<=1)return currentTime>=endTime?endValue:startValue;var ret=startValue+(currentTime-startTime)/(endTime-startTime)*(endValue-startValue);return Math.Clamp(ret,Math.Min(startValue,endValue),Math.Max(startValue,endValue));}}
class Reference {
 const int RimWidth=1;const uint OutsideContentRangeColor=0xff020202,BackgroundColor=0xff1e1e1e,EditorAreaColor=0xff303030,EditorAreaMouseOverColor=0xff373737,EditorAreaSelectedColor=0xff3e3e3e,ContentMarkerColor=0xffffffff;
 BoundsData Bounds=new();int VisibleHeight=16;Buffer RenderTarget=new();bool QuantizePositions,Grabbed,MouseOverEditor;double FullAreaStart,FullAreaEnd,EditorAreaStart,EditorAreaEnd,MiniMapAreaRange,MiniMapAreaStart,MiniMapAreaStartUnquantized,CursorPosition,HeightOverMiniMapAreaRange,FadeOutPercentage=0.9;ArrowGraphicManager ArrowGraphicManager;
 uint[] ClearData=new uint[256],ClearDataOutsideContentArea=new uint[256],ClearDataEditorSelectedArea=new uint[256],ClearDataEditorMouseOverArea=new uint[256],ClearDataEditorArea=new uint[256];List<double> Markers=new();
 int AddHorizontalLine(double y,int x,uint width,uint color,bool force=false){Markers.Add(y);return 0;}
 int AddCursor(double position){Markers.Add(GetYPixelRelativeToBounds(position,true));return 0;}
 public object Run(Input input) {
  QuantizePositions=input.quantize;Grabbed=input.state=="grabbed";MouseOverEditor=input.state=="hover";
  for(int y=0;y<16;y++)for(int x=0;x<16;x++){int i=y*16+x;bool rim=x==0||x==15||y==0||y==15;ClearData[i]=rim?0xffffffff:BackgroundColor;ClearDataOutsideContentArea[i]=rim?0xffffffff:OutsideContentRangeColor;ClearDataEditorArea[i]=rim?0xffffffff:EditorAreaColor;ClearDataEditorMouseOverArea[i]=rim?0xffffffff:EditorAreaMouseOverColor;ClearDataEditorSelectedArea[i]=rim?0xffffffff:EditorAreaSelectedColor;}
  UpdateBegin(-8,40,0,24,16,input.editorStart,input.editorEnd,3,new ArrowGraphicManager());
  var before=(uint[])RenderTarget.Data.Clone();UpdateEnd();return new{miniStart=MiniMapAreaStart,miniUnquantized=MiniMapAreaStartUnquantized,markers=Markers,before,after=RenderTarget.Data,drawable=GetEditorAreaPercentageOfMiniMapArea()<1.0};
 }
'''+ '\n'.join(body for name,body in methods.items() if name!='ColorRGBAInterpolateBGR')+'''
 static void Main(){var inputs=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),new JsonSerializerOptions{IncludeFields=true});Console.Write(JsonSerializer.Serialize(Array.ConvertAll(inputs,i=>new Reference().Run(i))));}
}'''
 with tempfile.TemporaryDirectory(prefix='deadsync-minimap-background-') as folder:
  root=Path(folder);(root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
  build=subprocess.run([str(a.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],text=True,capture_output=True)
  if build.returncode:raise SystemExit(build.stdout+build.stderr)
  run=subprocess.run([str(a.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),text=True,capture_output=True,check=True)
 expected=json.loads(run.stdout);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(dict(reference_commit=PIN,source_sha256=hashlib.sha256(raw).hexdigest(),method_sha256={name:hashlib.sha256(body.encode()).hexdigest() for name,body in methods.items()},scope='Unchanged UpdateBegin/UpdateEnd plus projection; clear buffers initialized by source rim policy, marker calls recorded not drawn, interpolation adapter preserves source arithmetic.',cases=[dict(input=i,expected=e) for i,e in zip(cases,expected)]),indent=2)+'\n');print(f'Generated {len(cases)} original background/fade cases')
if __name__=='__main__':main()
