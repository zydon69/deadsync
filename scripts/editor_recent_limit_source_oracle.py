#!/usr/bin/env python3
"""Pinned Editor.UpdateRecentFilesForActiveSong unchanged body + real Fumen action.
Song/pack/chart/zoom descriptors are deterministic host adapters; this proves list
ordering/truncation and limit-only Undo, not SavedSongInformation metadata or UI.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN="48e18f71655d0f0d86144c63239331833501fda2"
FPIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
def show(root,pin,path):return subprocess.check_output(['git','-C',str(root),'show',pin+':'+path]).decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);opening=text.index('{',start);depth=1;i=opening+1
 while depth:
  depth+=(text[i]=='{')-(text[i]=='}');i+=1
 return text[start:i]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 source=show(a.source,PIN,'StepManiaEditor/Editor.cs');body=member(source,'private void UpdateRecentFilesForActiveSong()')
 options=show(a.source,PIN,'StepManiaEditor/Preferences/PreferencesOptions.cs')
 assert 'public int RecentFilesHistorySize' in options
 dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
 host=r'''using System;using System.Collections.Generic;using System.Linq;using System.Text.Json;using Fumen;
class Options{public int RecentFilesHistorySize=4;}
class Preferences{public static Preferences Instance=new();public Options PreferencesOptions=new();public List<SavedSongInformation> RecentFiles=new();}
class SavedSongInformation{public string FileName;public SavedSongInformation(string f,string title=null,string pack=null,double z=0,double position=0,object charts=null,object focused=null){FileName=f;}}
class Song{public string Title="same";public string Path;public string GetFileFullPath()=>Path;}
class Pack{public string GetPackName()=>"pack";}
class Position{public double ChartPosition=0;}
class Zoom{public double GetSpacingZoom()=>1;}
class Editor{public Song ActiveSong;public Pack ActivePack;public object ActiveChartData;public Zoom ZoomManager=new();public Position GetPosition()=>new();public object GetFocusedChartData()=>null;
BODY
public void Record(string name){ActiveSong=name==null?null:new Song{Path=name};UpdateRecentFilesForActiveSong();}}
class Program{static void Main(){var p=Preferences.Instance;var output=new List<object>();void Capture(string name){output.Add(new{name,maximum=p.PreferencesOptions.RecentFilesHistorySize,files=p.RecentFiles.Select(x=>x.FileName).ToArray()});}foreach(var name in new[]{"A","B","C","D"})p.RecentFiles.Add(new(name));Capture("before");var action=new ActionSetObjectFieldOrPropertyValue<int>(p.PreferencesOptions,"RecentFilesHistorySize",2,false);action.Do();Capture("reduce");action.Undo();Capture("undo");action.Do();Capture("redo");var e=new Editor();e.Record(null);Capture("no-active");e.Record("C");Capture("record-C");action.Undo();Capture("undo-after-record");action.Do();Capture("redo-after-record");var zero=new ActionSetObjectFieldOrPropertyValue<int>(p.PreferencesOptions,"RecentFilesHistorySize",0,false);zero.Do();Capture("zero");e.Record("E");Capture("record-zero");zero.Undo();Capture("undo-zero");Console.Write(JsonSerializer.Serialize(new{cases=output}));}}
'''.replace('BODY',body)
 with tempfile.TemporaryDirectory(prefix='deadsync-mru-oracle-')as t:
  w=Path(t);(w/'Program.cs').write_text(host);(w/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+str(dll)+'</HintPath></Reference></ItemGroup></Project>');subprocess.run([str(a.dotnet),'build',str(w/'Oracle.csproj'),'-o',str(w/'out'),'--nologo'],check=True,stdout=subprocess.DEVNULL);data=subprocess.check_output([str(a.dotnet),str(w/'out/Oracle.dll')]);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(json.loads(data),indent=2)+'\n')
 a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=PIN,fumen_commit=FPIN,source_sha256=hashlib.sha256(source.encode()).hexdigest(),options_sha256=hashlib.sha256(options.encode()).hexdigest(),method_sha256=hashlib.sha256(body.encode()).hexdigest(),host_sha256=hashlib.sha256(host.encode()).hexdigest(),fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),scope=__doc__),indent=2)+'\n')
if __name__=='__main__':main()
