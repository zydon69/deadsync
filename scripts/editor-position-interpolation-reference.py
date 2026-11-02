#!/usr/bin/env python3
"""Run the entire pinned EditorPosition against the pinned, built Fumen assembly.
Chart stub is linear and explicitly does not certify timing-event projection.
"""
import argparse, subprocess, tempfile, json, hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2'
fumen_pin='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
assert subprocess.check_output(['git','-C',str(a.reference_root/'Fumen'),'rev-parse','HEAD'],text=True).strip()==fumen_pin
src=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/EditorPosition.cs'],text=True,encoding='utf-8-sig')
program=r'''using System;using System.Collections.Generic;using System.Reflection;using System.Text.Json;using StepManiaEditor;
namespace StepManiaEditor {
class Options {public double NewSongSyncOffset=0;}
class Scroll {public double ScrollInterpolationDuration=.1;}
class Preferences {public static Preferences Instance=new();public Options PreferencesOptions=new();public Scroll PreferencesScroll=new();}
class EditorSong {public double SyncOffset=0;}
class EditorChart {public double GetMusicOffset()=>0;public EditorSong GetEditorSong()=>new();public bool TryGetChartPositionFromTime(double t,ref double r){r=t;return true;}public bool TryGetTimeFromChartPosition(double r,ref double t){t=r;return true;}}
}
class Program {static void Main(){var cases=new List<object>();foreach(bool time in new[]{true,false})foreach(string kind in new[]{"ordinary","desired_ulp","setter_ulp","zero_duration","negative","cancel","axis_switch"}){
var pos=new EditorPosition(null,new EditorChart());pos.SongTime=1;pos.SetDesiredPositionToCurrent();var trace=new List<object>();
void Begin(double start,double delta){if(time)pos.BeginSongTimeInterpolation(start,delta);else pos.BeginChartPositionInterpolation(start,delta);}
void Update(double now){if(time)pos.UpdateSongTimeInterpolation(now);else pos.UpdateChartPositionInterpolation(now);var f=typeof(EditorPosition).GetField(time?"DesiredSongTime":"DesiredChartPosition",BindingFlags.Instance|BindingFlags.NonPublic);trace.Add(new{now,current=BitConverter.DoubleToInt64Bits(time?pos.SongTime:pos.ChartPosition),desired=BitConverter.DoubleToInt64Bits((double)f.GetValue(pos)),active=time?pos.IsInterpolatingSongTime():pos.IsInterpolatingChartPosition()});}
Preferences.Instance.PreferencesScroll.ScrollInterpolationDuration=kind=="zero_duration"?0:.1;
Begin(0,kind=="desired_ulp"?Math.BitIncrement(1)-1:kind=="negative"?-2:1);
Update(kind=="setter_ulp"?1e-17:.025);Update(.05);
if(kind=="cancel")pos.CancelInterpolating();
if(kind=="axis_switch"){if(time){pos.BeginChartPositionInterpolation(.05,2);pos.UpdateChartPositionInterpolation(.075);}else{pos.BeginSongTimeInterpolation(.05,2);pos.UpdateSongTimeInterpolation(.075);}}
Update(.1);Begin(.1,1);Update(.125);Update(1);cases.Add(new{time,kind,trace});}
Console.Write(JsonSerializer.Serialize(cases));}}
'''
with tempfile.TemporaryDirectory(prefix='position-source-oracle-') as temp:
 d=Path(temp);(d/'EditorPosition.cs').write_text(src);(d/'Program.cs').write_text(program)
 dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
 (d/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><LangVersion>preview</LangVersion></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
 r=subprocess.run([str(a.dotnet),'build',str(d/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],capture_output=True,text=True)
 if r.returncode:raise SystemExit(r.stdout+r.stderr)
 result=json.loads(subprocess.check_output([str(a.dotnet),str(d/'bin/Release/net10.0/Oracle.dll')],text=True))
 a.output.write_text(json.dumps({'source':pin,'editor_position_sha256':hashlib.sha256(src.encode()).hexdigest(),'fumen_source':fumen_pin,'fumen_dll_sha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'chart_adapter':'identity; no timing projection claim','cases':result},indent=2)+'\n')
 print(f'{len(result)} full original EditorPosition cases executed')
