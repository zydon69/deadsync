#!/usr/bin/env python3
"""Unchanged Editor.UpdateMiniMapBounds and UI scaling; supplied chart edges.
Does not execute original chart/waveform/misc-event geometry or native GPU draw.
"""
import argparse,hashlib,importlib.util,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 spec=importlib.util.spec_from_file_location('helper',Path(__file__).with_name('editor-minimap-content-oracle.py'));h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
 sources={};methods={}
 for path,names in {'StepManiaEditor/Editor.cs':['UpdateMiniMapBounds','TransformChartSpaceYToScreenSpaceY'],'StepManiaEditor/ImGui/ImGuiUtils.cs':['UiScaled','GetMiniMapYPaddingFromTopInScreenSpace','GetMiniMapYPaddingFromTopInChartSpace','GetMiniMapYPaddingFromBottom']}.items():
  raw=(a.reference_root/path).read_bytes()
  if raw!=subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{PIN}:{path}']):raise SystemExit('Source bytes differ from pin: '+path)
  sources[path]=hashlib.sha256(raw).hexdigest();source=raw.decode('utf-8-sig');methods.update({name:h.extract(source,name) for name in names})
 cases=[]
 for mount in ['RightSideOfWindow','LeftSideOfWindow','FocusedChartWithoutScaling','FocusedChartWithScaling']:
  for scale in [1.0,1.25,1.5,2.0]:
   for area,viewport in [([0,20,854,460],480),([37,80,640,300],400),([0,0,320,20],20)]:
    for focused in [False,True]:
     for offset in [-25,10,110]:
      for width in [0,90]:cases.append(dict(mount=mount,scale=scale,area=area,viewport=viewport,focused=focused,offset=offset,width=width,edge=500,scaledEdge=620))
 program='''using System;using System.Text.Json;
enum Position {RightSideOfWindow,LeftSideOfWindow,FocusedChartWithoutScaling,FocusedChartWithScaling}
class Input {public string mount;public float scale;public int[] area;public int viewport,offset,width,edge,scaledEdge;public bool focused;}
struct Rectangle {public int X,Y,Width,Height;public Rectangle(int x,int y,int width,int height){X=x;Y=y;Width=width;Height=height;}}
class Preferences {public static Preferences Instance=new();public MiniPrefs PreferencesMiniMap=new();}
class MiniPrefs {public Position MiniMapPosition;public int Offset,Width;public int GetPositionOffsetUiScaled()=>Editor.UiScaled(Offset);public int GetMiniMapWidthScaled()=>Editor.UiScaled(Width);}
class Focus {public int Normal,Scaled;public int GetScreenSpaceXOfMiscEventsEnd()=>Normal;public int GetScreenSpaceXOfMiscEventsEndWithCurrentScale()=>Scaled;}
class Target {public Rectangle Bounds;public uint Visible;public void UpdateBounds(object device,Rectangle bounds,uint visible){Bounds=bounds;Visible=visible;}}
class Editor {
 const int MenuBarHeightDefaultDPI=20,ChartHeaderHeightDefaultDPI=21,SceneWidgetPaddingDefaultDPI=10;
 static float Dpi;static float GetDpiScale()=>Dpi;Rectangle ChartArea;int Viewport;Focus focus;Target MiniMap=new();object GraphicsDevice=new();
 Focus GetFocusedChartData()=>focus;int GetViewportHeight()=>Viewport;
 public object Run(Input input){Dpi=input.scale;ChartArea=new Rectangle(input.area[0],input.area[1],input.area[2],input.area[3]);Viewport=input.viewport;focus=input.focused?new Focus{Normal=input.edge,Scaled=input.scaledEdge}:null;var p=Preferences.Instance.PreferencesMiniMap;p.MiniMapPosition=Enum.Parse<Position>(input.mount);p.Offset=input.offset;p.Width=input.width;UpdateMiniMapBounds();return new{x=MiniMap.Bounds.X,y=MiniMap.Bounds.Y,width=MiniMap.Bounds.Width,textureHeight=MiniMap.Bounds.Height,visibleHeight=MiniMap.Visible};}
'''+ '\n'.join(methods.values())+'''
 static void Main(){var inputs=JsonSerializer.Deserialize<Input[]>(Console.In.ReadToEnd(),new JsonSerializerOptions{IncludeFields=true});Console.Write(JsonSerializer.Serialize(Array.ConvertAll(inputs,i=>new Editor().Run(i))));}
}'''
 with tempfile.TemporaryDirectory(prefix='deadsync-minimap-mount-') as folder:
  root=Path(folder);(root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
  build=subprocess.run([str(a.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if build.returncode:raise SystemExit(build.stdout+build.stderr)
  run=subprocess.run([str(a.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),capture_output=True,text=True,check=True)
 expected=json.loads(run.stdout);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(dict(reference_commit=PIN,source_sha256=sources,method_sha256={name:hashlib.sha256(body.encode()).hexdigest() for name,body in methods.items()},scope='Unchanged bounds/scaling methods with supplied chart rectangle, focused misc edges and viewport; does not prove chart/waveform geometry, native UI mapping or GPU.',cases=[dict(input=i,expected=e) for i,e in zip(cases,expected)]),indent=2)+'\n');print(f'Generated {len(cases)} pinned mount/scaling cases')
if __name__=='__main__':main()
