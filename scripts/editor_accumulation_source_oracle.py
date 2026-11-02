#!/usr/bin/env python3
"""Execute pinned, unchanged GrooveAuthor preference groups and real Fumen.

Original full preference Load/PostLoad plus unchanged StepDensity rebuild/Add/Delete/breakdown methods and real Fumen IntervalTree/DynamicArray execute. Editor events are deterministic step/row adapters; clocks, drawing, observers and resource resizing are outside scope.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
EDITOR_PIN="48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
LIBRARY_PIN="bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
KEYS_SHA="26d5351258a363590c9e51af83283a35aec7f5036cb03cd8313500506dd8d6a6"
HOST='using System;using System.Numerics;\nnamespace StepManiaEditor.AutogenConfig{static class PerformedChartConfigManager{public static Guid DefaultPerformedChartConfigGuid=Guid.Parse("6276c906-ea8f-43b3-9500-0ddeac7bdc22");}}\nnamespace Microsoft.Xna.Framework{public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}public struct Rectangle{public int X,Y,Width,Height;}}\nnamespace StepManiaEditor{\nclass Editor{public enum NoteEntryMode{Normal,AdvanceBySnap}public enum SpacingMode{ConstantTime,ConstantRow,Variable}public enum WaveFormScrollMode{CurrentTempo,CurrentTempoAndRate,MostCommonTempo}public bool GetChartAreaInChartSpaceWithoutHeader(out Microsoft.Xna.Framework.Rectangle r){r=new();return false;}}\nclass SavedSongInformation{public string FullPath;public void PostLoad(){}}\nstatic class ImGuiUtils{public const int SceneWidgetPaddingDefaultDPI=10;public static int UiScaled(int value)=>value;}\nstatic class Utils{public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;}\nclass MiniMap{ENUMS}\n}\n'
PROGRAM='using System.Text.Json;using System.Reflection;using StepManiaEditor;\nclass Program{static void Main(){var options=(JsonSerializerOptions)typeof(Preferences).GetField("SerializationOptions",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=Path.GetTempFileName();try{File.WriteAllText(path,raw);var p=Preferences.Load(new Editor(),path);var stages=new List<object>();\n foreach(var scenario in new[]{"delete","add","chords"}){\n var rows=scenario=="delete"?new[]{0,12,24,36}:scenario=="add"?new[]{0,12,24}:new[]{0,0,12,12};var chart=new EditorChart(rows);var probe=new StepDensityProbe(chart);var before=probe.GetStreamBreakdown();\n var e=new EditorEvent(scenario=="delete"?36:scenario=="chords"?24:36);\n if(scenario=="delete"){chart.Events.RemoveAll(x=>x.GetRow()==36);probe.DeleteEvent(e,0);}else{chart.Events.Add(e);chart.Events.Sort((a,b)=>a.GetRow().CompareTo(b.GetRow()));probe.AddEvent(e,1);}\n var incremental=probe.GetStreamBreakdown();probe.Rebuild();var rebuilt=probe.GetStreamBreakdown();stages.Add(new{scenario,initial_rows=rows,before,incremental,rebuilt,rows_after=chart.Events.Select(e=>e.GetRow()).ToArray()});}\n output.Add(new{name=input.GetProperty("name").GetString(),raw,value=JsonSerializer.SerializeToElement(p,options),stages});}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{cases=output}));}}\n'
GROUPS=["Audio","WaveForm","Dark","Scroll","Selection","MiniMap","Options","Receptors","Performance","Stream","DensityGraph","NoteColor","KeyBinds"]
def git(root,*args): return subprocess.check_output(["git","-C",str(root),*args])
def original(root,path): return git(root,"show",EDITOR_PIN+":"+path)
def member(text,name):
 start=text.index("public enum "+name); begin=text.index("{",start); end=text.index("}",begin); return text[start:end+1]
def method(text, signature):
 start=text.index(signature);brace=text.index("{",start);depth=1;end=brace+1
 while depth:depth+=(text[end]=="{")-(text[end]=="}");end+=1
 return text[start:end]
def requests(defaults):
 return [dict(name="accum-"+str(v)+("-literal" if isinstance(v,int)else""),raw=json.dumps({"Player":3,"PreferencesStream":{"NoteType":"QuarterNotes","AccumulationType":v},"PreferencesDensityGraph":{"AccumulationType":v}}))for v in ["Step","Row","99","-99",99]]

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ["source","reference-root","dotnet","output"]:parser.add_argument("--"+name,type=Path,required=True)
 parser.add_argument("--keys",type=Path,required=True)
 args=parser.parse_args()
 assert git(args.source,"rev-parse","HEAD").decode().strip()==EDITOR_PIN
 assert git(args.reference_root,"rev-parse","HEAD").decode().strip()==LIBRARY_PIN
 fumen=args.reference_root/"Fumen";assert git(fumen,"rev-parse","HEAD").decode().strip()==FUMEN_PIN
 dll=fumen/"Fumen/bin/Release/net10.0/Fumen.dll"
 if not dll.exists(): raise SystemExit("Build the pinned Fumen net10.0 Release assembly first")
 keys=args.keys;keys_bytes=keys.read_bytes();assert hashlib.sha256(keys_bytes).hexdigest()==KEYS_SHA
 hashes={}
 with tempfile.TemporaryDirectory(prefix="deadsync-real-preference-groups-") as folder:
  work=Path(folder)
  for name in ["Preferences"]+["Preferences"+g for g in GROUPS]:
   path="StepManiaEditor/Preferences/"+name+".cs";raw=original(args.source,path);(work/(name+".cs")).write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  path="StepManiaEditor/SubdivisionType.cs";raw=original(args.source,path);(work/"SubdivisionType.cs").write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  mini_path="StepManiaEditor/MiniMap.cs";raw=original(args.source,mini_path);hashes[mini_path]=hashlib.sha256(raw).hexdigest();mini=raw.decode("utf-8-sig")
  imgui_path="StepManiaEditor/ImGui/ImGuiUtils.cs";raw=original(args.source,imgui_path);hashes[imgui_path]=hashlib.sha256(raw).hexdigest();assert "public const int SceneWidgetPaddingDefaultDPI = 10;" in raw.decode("utf-8-sig")
  density=original(args.source,"StepManiaEditor/StepDensity.cs");utils=original(args.source,"StepManiaEditor/Utils.cs");hashes["StepManiaEditor/StepDensity.cs"]=hashlib.sha256(density).hexdigest();hashes["StepManiaEditor/Utils.cs"]=hashlib.sha256(utils).hexdigest()
  text=density.decode("utf-8-sig")
  bodies="\n".join(method(text,k)for k in ["public readonly struct Measure","private class StreamSegment","public string GetStreamBreakdown()","public void AddEvent(EditorEvent editorEvent, int numStepsAtRow)","public void DeleteEvent(EditorEvent editorEvent, int numStepsAtRow)","private void RecomputeStreams(bool initializeMeasureSteps)","private static int GetMeasureNumber(EditorEvent editorEvent)","private static int GetMeasureNumber(int row)"])
  getter="\n".join(method(utils.decode("utf-8-sig"),k)for k in ["public static int GetBeatSubdivision(SubdivisionType subdivisionType)","public static int GetMeasureSubdivision(SubdivisionType subdivisionType)"])
  host=HOST.replace("ENUMS",member(mini,"Position")+member(mini,"SelectMode")).replace("public static uint ToRGBA(float r,float g,float b,float a)=>0;",getter+"public static uint ToRGBA(float r,float g,float b,float a)=>0;")
  host="using System.Text;using System.Diagnostics;using Fumen;using Fumen.Converters;using static Fumen.Converters.SMCommon;using static StepManiaEditor.Utils;\n"+host
  host+="\nnamespace StepManiaEditor {class EditorEvent {int row;public EditorEvent(int r){row=r;}public int GetRow()=>row;public bool IsStep()=>true;public bool IsFake()=>false;}class EditorLastSecondHintEvent:EditorEvent{public EditorLastSecondHintEvent():base(0){}}class EditorChart{public List<EditorEvent> Events;public EditorChart(int[] rows){Events=rows.Select(r=>new EditorEvent(r)).ToList();}public IEnumerable<EditorEvent> GetEvents()=>Events;}class StepDensityProbe{readonly EditorChart EditorChart;readonly DynamicArray<Measure> Measures=new(256);readonly int[] MeasuresByStepCount=new int[RowsPerMeasure];IntervalTree<int,StreamSegment> Streams=new();string StreamBreakdown;bool StreamBreakdownDirty=true;public StepDensityProbe(EditorChart c){EditorChart=c;for(int i=0;i<8;i++)Measures.Add(new Measure(i*2,0,0));RecomputeStreams(true);}void ResizeMeasures(){}public void Rebuild(){RecomputeStreams(false);}"+bodies+"}}"
  (work/"Host.cs").write_text(host);(work/"Keys.cs").write_bytes(keys_bytes);(work/"Program.cs").write_text(PROGRAM)
  (work/"Oracle.csproj").write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
  run=subprocess.run([str(args.dotnet),"build",str(work/"Oracle.csproj"),"-c","Release","--nologo","-v","q","-p:WarningLevel=0"],capture_output=True,text=True)
  if run.returncode:raise SystemExit(run.stdout+run.stderr)
  assembly=work/"bin/Release/net10.0/Oracle.dll"
  def execute(request):return json.loads(subprocess.run([str(args.dotnet),str(assembly)],input=json.dumps(request),capture_output=True,text=True,check=True).stdout)
  defaults={}
  payload=execute(requests(defaults))
  for case in payload["cases"]:
   case["accepted"]=case["name"]=="constructors-postload" or case["name"].endswith("-empty") or case["value"]["Player"]==3
   case["value"]={k:case["value"][k]for k in ["PreferencesStream","PreferencesDensityGraph"]}
  provenance=dict(editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sources=hashes,host_sha256=hashlib.sha256(host.encode()).hexdigest(),program_sha256=hashlib.sha256(PROGRAM.encode()).hexdigest(),keys_sha256=KEYS_SHA,assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),scope=__doc__)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(payload,indent=2)+"\n");args.output.with_suffix(".reference.json").write_text(json.dumps(provenance,indent=2)+"\n");print("Executed",len(payload["cases"]),"original stream rebuild/incremental transcripts")
if __name__=="__main__": main()
