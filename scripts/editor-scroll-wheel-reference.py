#!/usr/bin/env python3
"""Execute unchanged GA wheel and ZoomManager input methods against real Fumen."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for k in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+k,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2'
def read(rel):return subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':'+rel],text=True,encoding='utf-8-sig')
def method(s,sig):
 start=s.index(sig);pos=s.index('{',start)+1;level=1
 while level:
  level+=(s[pos]=='{')-(s[pos]=='}');pos+=1
 return s[start:pos]
wheel=method(read('StepManiaEditor/Editor.cs'),'private void ProcessInputForScrollingAndZooming(double currentTime, double deltaTime)')
zoom=method(read('StepManiaEditor/ZoomManager.cs'),'public bool ProcessInput(double currentTime, KeyCommandManager keyCommandManager, float scrollDelta)')
program=r'''using System.Text.Json;using Fumen;using static Editor;
class PrefScroll {public SpacingMode SpacingMode;public double ZoomMultiplier=1.2,ScrollWheelTime=.25;public int ScrollWheelRows=48;public bool StopPlaybackWhenScrolling,Reverse;}
class PrefKeys {public int ScrollZoom=1,ScrollSpacing=2;}
class PrefAudio {public double MusicRate=1;}
class Preferences {public static Preferences Instance=new();public PrefScroll PreferencesScroll=new();public PrefKeys PreferencesKeyBinds=new();public PrefAudio PreferencesAudio=new();}
class KeyCommandManager {public bool Zoom,Spacing;public bool IsAnyInputDown(int id)=>id==1?Zoom:Spacing;}
class Data {public string Name;public List<object> Trace;public Data(string n,List<object> t){Name=n;Trace=t;}public void StartInterpolation(double t,double m)=>Trace.Add(new{kind=Name,time=t,multiplier=m});}
class ZoomManager {public double SpacingZoom=1;public Data ZoomData,ConstantTimeSpacingData,ConstantRowSpacingData,VariableSpacingData;public ZoomManager(List<object> t){ZoomData=new("Zoom",t);ConstantTimeSpacingData=new("TimeSpacing",t);ConstantRowSpacingData=new("RowSpacing",t);VariableSpacingData=new("VariableSpacing",t);}public double GetSpacingZoom()=>SpacingZoom;__ZOOM__}
class Mouse {public int Delta;public int ScrollDeltaSinceLastFrame()=>Delta;public int GetDefaultScrollDetentValue()=>120;}
class Position {public List<object> Trace;public double SongTime;public Position(List<object> t){Trace=t;}public void BeginSongTimeInterpolation(double t,double d)=>Trace.Add(new{kind="Time",time=t,delta=d});public void BeginChartPositionInterpolation(double t,double d)=>Trace.Add(new{kind="Rows",time=t,delta=d});}
class Chart {public Position Position;public Chart(List<object> t){Position=new(t);}}
class Timer {public TimeSpan Elapsed=>TimeSpan.FromSeconds(2);}
class Music {public void SetMusicTimeInSeconds(double t){}}
class Editor {public enum SpacingMode {ConstantTime,ConstantRow,Variable}public List<object> Trace=new();public Mouse EditorMouseState=new();public KeyCommandManager KeyCommandManager=new();public ZoomManager ZoomManager;public Chart Chart;public bool Playing;double PlaybackStartTime;Timer PlaybackStopwatch=new();Music MusicManager=new();public Snap SnapManager=new();public Editor(){Chart=new(Trace);ZoomManager=new(Trace);}Chart GetFocusedChartData()=>Chart;double GetScrollDirectionSign()=>Preferences.Instance.PreferencesScroll.Reverse?-1:1;void SetSongTime(double time){Chart.Position.SongTime=time;Trace.Add(new{kind="PlayingTime",time});}Position GetPosition()=>Chart.Position;void StopPlayback()=>Trace.Add(new{kind="Stop"});void UpdateAutoPlayFromScrolling(){}void OnMoveUp()=>Trace.Add(new{kind=Preferences.Instance.PreferencesScroll.Reverse?"Later":"Earlier"});void OnMoveDown()=>Trace.Add(new{kind=Preferences.Instance.PreferencesScroll.Reverse?"Earlier":"Later"});public void Run()=>ProcessInputForScrollingAndZooming(10,.025);__WHEEL__}
class Snap {public int Rows;public int GetCurrentRows()=>Rows;}
class Program {static void Main(){var output=new List<object>();foreach(var mode in new[]{0,1,2,99})foreach(var modifiers in new[]{0,1,2,3})foreach(var playing in new[]{false,true})foreach(var snapped in new[]{false,true})foreach(var reverse in new[]{false,true})foreach(var delta in new[]{-240,1,120}){Preferences.Instance=new();var p=Preferences.Instance.PreferencesScroll;p.SpacingMode=(SpacingMode)mode;p.Reverse=reverse;var e=new Editor();e.Playing=playing;e.KeyCommandManager.Zoom=(modifiers&1)!=0;e.KeyCommandManager.Spacing=(modifiers&2)!=0;e.SnapManager.Rows=snapped?48:0;e.EditorMouseState.Delta=delta;e.ZoomManager.SpacingZoom=2;e.Run();output.Add(new{mode,modifiers,playing,snapped,reverse,delta,trace=e.Trace});}Console.Write(JsonSerializer.Serialize(output));}}
'''.replace('__ZOOM__',zoom).replace('__WHEEL__',wheel)
with tempfile.TemporaryDirectory(prefix='wheel-oracle-') as temp:
 d=Path(temp);(d/'Program.cs').write_text(program);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
 (d/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(d/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True,capture_output=True)
 result=json.loads(subprocess.check_output([str(a.dotnet),str(d/'bin/Release/net10.0/Oracle.dll')],text=True))
 a.output.write_text(json.dumps({'source':pin,'wheel_sha256':hashlib.sha256(wheel.encode()).hexdigest(),'zoom_sha256':hashlib.sha256(zoom.encode()).hexdigest(),'cases':result},indent=2)+'\n');print(f'{len(result)} unchanged-source wheel cases executed')
