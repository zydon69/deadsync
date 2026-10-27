#!/usr/bin/env python3
"""Execute unchanged source selection/drag methods with a supplied map rectangle.
No native keyboard, timing inversion or GPU equivalence is claimed.
"""
import argparse,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 spec=importlib.util.spec_from_file_location('helper',Path(__file__).with_name('editor-minimap-content-oracle.py'));h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
 path='StepManiaEditor/MiniMap.cs';raw=(a.reference_root/path).read_bytes()
 if raw!=subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{PIN}:{path}']):raise SystemExit('Source bytes differ from pin')
 names=['ReflectScreenYIfNeeded','MouseDownInternal','MouseMoveInternal','UpdateBasedOnSelectedPixel','SetMiniMapAreaFromEditorArea','SetMiniMapAreaStart','GetYPixelRelativeToBounds','GetYPixelRelativeToScreen','GetPositionRelativeToPixel','IsScreenPositionInMiniMapBoundsInternal','IsScreenPositionInEditorBounds']
 methods={name:h.extract(raw.decode('utf-8-sig'),name) for name in names}
 cases=[]
 for mode in ['MoveToCursor','MoveToSelectedArea']:
  for reverse in [False,True]:
   for quantize in [False,True]:
    for full,mini,editor in [(1000.,400.,80.),(200.,400.,80.),(1000.,400.,400.)]:
     for pixel in [19,20,45,70,100,170,220,221]:
      for cursor in [.25,.75]:
       cases.append(dict(mode=mode,reverse=reverse,quantize=quantize,full=full,mini=mini,editor=editor,pixel=pixel,cursor=cursor))
 program='''using System;using System.Text.Json;
class Input {public string mode;public bool reverse,quantize;public double full,mini,editor,cursor;public int pixel;}
class Reference {
 enum SelectMode {MoveToCursor,MoveToSelectedArea}
 struct Rect {public int X,Y,Width;}
 Rect Bounds;int VisibleHeight;bool Reverse,QuantizePositions,MouseOverEditor,Grabbed;
 double FullAreaStart,FullAreaEnd,MiniMapAreaStart,MiniMapAreaStartUnquantized,MiniMapAreaRange,HeightOverMiniMapAreaRange;
 double EditorAreaStart,EditorAreaEnd,GrabbedPositionAsPercentageOfEditorArea,CursorPosition;
 SelectMode EditorSelectMode;
 object Snapshot()=>new{grabbed=Grabbed,percentage=GrabbedPositionAsPercentageOfEditorArea,miniStart=MiniMapAreaStart,miniUnquantized=MiniMapAreaStartUnquantized,editorStart=EditorAreaStart,editorEnd=EditorAreaEnd};
 public object Run(Input i){Bounds=new Rect{X=0,Y=20,Width=10};VisibleHeight=200;Reverse=i.reverse;QuantizePositions=i.quantize;FullAreaStart=-10;FullAreaEnd=i.full-10;MiniMapAreaRange=i.mini;HeightOverMiniMapAreaRange=VisibleHeight/MiniMapAreaRange;EditorAreaStart=23.75;EditorAreaEnd=EditorAreaStart+i.editor;CursorPosition=EditorAreaStart+i.editor*i.cursor;EditorSelectMode=Enum.Parse<SelectMode>(i.mode);SetMiniMapAreaStart(-3.25);var accepted=MouseDownInternal(5,ReflectScreenYIfNeeded(i.pixel));var begin=Snapshot();MouseMoveInternal(5,ReflectScreenYIfNeeded(i.pixel+15));return new{accepted,begin,moved=Snapshot()};}
'''+ '\n'.join(methods.values())+'''
 static void Main(){var inputs=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),new JsonSerializerOptions{IncludeFields=true});Console.Write(JsonSerializer.Serialize(Array.ConvertAll(inputs,i=>new Reference().Run(i))));}}
'''
 with tempfile.TemporaryDirectory(prefix='deadsync-minimap-select-') as folder:
  root=Path(folder);(root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
  build=subprocess.run([str(a.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if build.returncode:raise SystemExit(build.stdout+build.stderr)
  result=subprocess.run([str(a.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),capture_output=True,text=True,check=True)
 values=json.loads(result.stdout);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(dict(reference_commit=PIN,source_sha256=hashlib.sha256(raw).hexdigest(),method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in methods.items()},scope=__doc__,cases=[dict(input=i,expected=e) for i,e in zip(cases,values)]),indent=2)+'\n');print(f'Generated {len(cases)} source selection cases')
if __name__=='__main__':main()
