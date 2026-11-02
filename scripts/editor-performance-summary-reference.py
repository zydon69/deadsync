#!/usr/bin/env python3
"""Execute unchanged UIPerformance with recording, noninteractive UI adapters."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

p = argparse.ArgumentParser()
for key in ['editor-root','reference-root','dotnet','output']:
    p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args()
pin='48e18f71655d0f0d86144c63239331833501fda2'
fpin='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for root, revision in [(a.editor_root,pin),(a.reference_root/'Fumen',fpin)]:
    assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==revision
paths=['PerformanceMonitor.cs','PerformanceTimings.cs','Preferences/PreferencesPerformance.cs','UI/UIPerformance.cs']
sources={path:subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/'+path]) for path in paths}
cases=[]
for durations in [[10_000_000,20_000_000,30_000_000],[2_000_000_000],[0,10_000_000]]:
    for count in [-3,0,1,2,512]:
        for mode in [0,1,2,99]:
            cases.append(dict(durations=durations,count=count,mode=mode))
adapter=r'''
using System.Diagnostics;
using System.Globalization;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Serialization;
using StepManiaEditor;
namespace ImGuiNET {
 enum ImGuiCol {FrameBg}
 static class ImGui {
  public static bool Button(string s)=>false;
  public static void Separator(){} public static void End(){}
  public static void PushStyleColor(ImGuiCol c,uint v){} public static void PopStyleColor(){}
 }
}
namespace StepManiaEditor {
 class Preferences {
  public static Preferences Instance=new();
  public PreferencesPerformance PreferencesPerformance=new();
 }
 abstract class UIWindow {
  protected string WindowTitle;protected UIWindow(string title){WindowTitle=title;}
  public virtual void Open(bool focus){} public virtual void Close(){}
  protected void Focus(){} protected bool BeginWindow(string title,ref bool show,int width)=>true;
 }
 static class ImGuiUtils {
  public static int UiScaled(int x)=>x;
  public static void PushDisabled(){} public static void PopDisabled(){}
 }
 static class ImGuiLayoutUtils {
  public static Dictionary<string,string> Labels=new();
  public static List<object> Plots=new();
  public static bool BeginTable(string title,int width)=>true;
  public static void EndTable(){}
  public static void DrawRowTitleAndText(string title,string value,string help){Labels[title]=value;}
  public static void DrawRowCheckbox(params object[] args){}
  public static void DrawRowEnum<T>(params object[] args){}
  public static void DrawRowDragDouble(params object[] args){}
  public static void DrawRowDragInt(params object[] args){}
  public static void DrawRowPlot(string title,ref float first,int count,string overlay,float max,
   float height,int stride,string description) {
   var span=System.Runtime.InteropServices.MemoryMarshal.CreateReadOnlySpan(ref first,(count-1)*stride/4+1);
   var data=new float[count];for(int i=0;i<count;i++)data[i]=span[i*stride/4];
   Plots.Add(new{title,count,overlay,max,description,data});
  }
 }
}
class Program {
 static object Field(string name)=>typeof(UIPerformance).GetField(name,BindingFlags.Instance|BindingFlags.NonPublic).GetValue(UIPerformance.Instance);
 static void Main(string[] args) {
  CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo("en-US");
  var input=JsonDocument.Parse(File.ReadAllText(args[0]));var output=new List<object>();
  foreach(var test in input.RootElement.EnumerateArray()) {
   var monitor=new PerformanceMonitor(1024,PerformanceTimings.PerfTimings);
   long ticks=100;monitor.BeginFrameFromStopWatch(ticks);int index=0;
   foreach(var duration in test.GetProperty("durations").EnumerateArray()) {
    for(int t=0;t<PerformanceTimings.PerfTimings.Length;t++)
     monitor.SetTimeFromStopWatch(PerformanceTimings.PerfTimings[t],(index+1)*(t+1)*100000L);
    ticks+=duration.GetInt64();monitor.BeginFrameFromStopWatch(ticks);index++;
   }
   Preferences.Instance.PreferencesPerformance=new PreferencesPerformance {
    ShowPerformanceWindow=true,MaxFramesToDraw=test.GetProperty("count").GetInt32(),
    FrameMaxTime=(PreferencesPerformance.FrameMaxTimeMode)test.GetProperty("mode").GetInt32(),
    ExplicitFrameMaxTime=.0125
   };
   ImGuiLayoutUtils.Labels=new();ImGuiLayoutUtils.Plots=new();
   UIPerformance.Instance.Init(monitor);UIPerformance.Instance.Draw();
   output.Add(new{input=test.Clone(),frequency=Stopwatch.Frequency,labels=ImGuiLayoutUtils.Labels,
    plots=ImGuiLayoutUtils.Plots,current=Field("TimingLastFrameValues"),
    averages=Field("TimingAverages"),maxima=Field("MaxTimePerTiming")});
  }
  Console.Write(JsonSerializer.Serialize(output,new JsonSerializerOptions{NumberHandling=JsonNumberHandling.AllowNamedFloatingPointLiterals}));
 }
}
'''
with tempfile.TemporaryDirectory(prefix='ga-performance-summary-') as temp:
    root=Path(temp)
    for path,source in sources.items():(root/Path(path).name).write_bytes(source)
    (root/'Program.cs').write_text(adapter)
    (root/'input.json').write_text(json.dumps(cases))
    dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
    (root/'Reference.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
    subprocess.run([str(a.dotnet),'build',str(root/'Reference.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
    result=json.loads(subprocess.check_output([str(a.dotnet),str(root/'bin/Release/net10.0/Reference.dll'),str(root/'input.json')],text=True))
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps(dict(revision=pin,fumen_revision=fpin,
    sources={path:hashlib.sha256(source).hexdigest() for path,source in sources.items()},
    adapter_sha256=hashlib.sha256(adapter.encode()).hexdigest(),
    scope='Whole original UIPerformance.Draw, original recorder and preference class; recording ImGui/layout adapters never change controls. No GPU rendering or host instrumentation executed.',
    cases=result),indent=2)+'\n')
