#!/usr/bin/env python3
"""Execute pinned, unchanged GrooveAuthor preference groups and real Fumen.

Only external UI/cache dependencies are hosted: chartless Editor, ImGui scale1,
SavedSong placeholder, and GPU Color/uint color cache plumbing. Their pixels,
recent-song behavior and runtime settings are not asserted by this oracle.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
EDITOR_PIN="48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
LIBRARY_PIN="bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
KEYS_SHA="26d5351258a363590c9e51af83283a35aec7f5036cb03cd8313500506dd8d6a6"
HOST='using System;using System.Numerics;\nnamespace StepManiaEditor.AutogenConfig{static class PerformedChartConfigManager{public static Guid DefaultPerformedChartConfigGuid=Guid.Parse("6276c906-ea8f-43b3-9500-0ddeac7bdc22");}}\nnamespace Microsoft.Xna.Framework{public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}public struct Rectangle{public int X,Y,Width,Height;}}\nnamespace StepManiaEditor{\nclass Editor{public enum NoteEntryMode{Normal,AdvanceBySnap}public enum SpacingMode{ConstantTime,ConstantRow,Variable}public enum WaveFormScrollMode{CurrentTempo,CurrentTempoAndRate,MostCommonTempo}public bool GetChartAreaInChartSpaceWithoutHeader(out Microsoft.Xna.Framework.Rectangle r){r=new();return false;}}\nclass SavedSongInformation{public string FullPath;public void PostLoad(){}}\nstatic class ImGuiUtils{public const int SceneWidgetPaddingDefaultDPI=10;public static int UiScaled(int value)=>value;}\nstatic class Utils{public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;}\nclass MiniMap{ENUMS}\n}\n'
PROGRAM='using System.Text.Json;using System.Text.Json.Serialization;using System.Reflection;using StepManiaEditor;\nclass Program{\n static void Main(){var options=(JsonSerializerOptions)typeof(Preferences).GetField("SerializationOptions",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=Path.GetTempFileName();try{File.WriteAllText(path,raw);var p=Preferences.Load(new Editor(),path);output.Add(new{name=input.GetProperty("name").GetString(),raw,value=JsonSerializer.SerializeToElement(p,options)});}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{processor_count=Environment.ProcessorCount,cases=output}));}\n}\n'
GROUPS=["Audio","WaveForm","Dark","Scroll","Selection","MiniMap","Options","Receptors","Performance","Stream","DensityGraph","NoteColor","KeyBinds"]
def git(root,*args): return subprocess.check_output(["git","-C",str(root),*args])
def original(root,path): return git(root,"show",EDITOR_PIN+":"+path)
def member(text,name):
 start=text.index("public enum "+name); begin=text.index("{",start); end=text.index("}",begin); return text[start:end+1]
def requests(defaults):
 result=[dict(name="constructors-postload",raw="{}")]
 groups={k:v for k,v in defaults.items() if k.startswith("Preferences") and k!="PreferencesKeyBinds"}
 for group,fields in groups.items():
  result.append(dict(name=group+"-empty",raw=json.dumps({group:{}})))
  for field,value in fields.items():
   if isinstance(value,str) and field not in ["ShortBreakCharacter","LongBreakCharacter"]:
    for name,probe in [("unknown-name","future"),("non-string",1),("numeric-string","1"),("unknown-ordinal","99")]: result.append(dict(name=group+"."+field+"-"+name,raw=json.dumps({group:{field:probe},"Player":3})))
   if isinstance(value,dict) and all(k in ["X","Y","Z","W"] for k in value):result.append(dict(name=group+"."+field+"-partial-vector",raw=json.dumps({group:{field:{"X":0.2}},"Player":3})))
 for group,field in [("PreferencesMiniMap","PositionOffsets"),("PreferencesDensityGraph","DensityGraphPositionOffsets"),("PreferencesDensityGraph","DensityGraphWidthOffsets")]:
  for name,probe in [("empty",{}),("partial",{"LeftSideOfWindow":123}),("aliases",{"leftsideofwindow":124,"1":125}),("unknown",{"future":123,"99":124})]:result.append(dict(name=group+"."+field+"-"+name,raw=json.dumps({group:{field:probe},"Player":3})))
 for name,raw in [("wave-postload-zero",{"PreferencesWaveForm":{"WaveFormLoadingMaxParallelism":0}}),("wave-postload-negative",{"PreferencesWaveForm":{"WaveFormLoadingMaxParallelism":-1}}),("receptors-negative",{"PreferencesReceptors":{"PositionX":-20,"PositionY":-10}}),("options-startup-empty",{"PreferencesOptions":{"StartupStepGraphs":[]}})]:result.append(dict(name=name,raw=json.dumps(dict(raw,Player=3))))
 for group,fields in groups.items():
  changed={}
  for key,value in fields.items():
   if isinstance(value,bool):changed[key]=not value
   elif isinstance(value,(int,float)):changed[key]=value+3 if isinstance(value,int) else 0.3
   elif isinstance(value,str) and key in ["ShortBreakCharacter","LongBreakCharacter"]:changed[key]="/"
   elif isinstance(value,list):changed[key]=value[:1]
   elif isinstance(value,dict):changed[key]={k:(0.2 if k in ["X","Y","Z","W"] else v+3) for k,v in value.items()}
  result.append(dict(name=group+"-business-fields",raw=json.dumps({group:changed,"Player":3})))
 result.append(dict(name="stream-char-emoji",raw=json.dumps({"Player":3,"PreferencesStream":{"ShortBreakCharacter":"😀"}})))
 result.append(dict(name="stream-char-linebreak",raw=json.dumps({"Player":3,"PreferencesStream":{"ShortBreakCharacter":"\n"}})))
 return result

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ["source","reference-root","dotnet","output"]:parser.add_argument("--"+name,type=Path,required=True)
 args=parser.parse_args()
 assert git(args.source,"rev-parse","HEAD").decode().strip()==EDITOR_PIN
 assert git(args.reference_root,"rev-parse","HEAD").decode().strip()==LIBRARY_PIN
 fumen=args.reference_root/"Fumen";assert git(fumen,"rev-parse","HEAD").decode().strip()==FUMEN_PIN
 dll=fumen/"Fumen/bin/Release/net10.0/Fumen.dll"
 if not dll.exists(): raise SystemExit("Build the pinned Fumen net10.0 Release assembly first")
 keys=Path(__file__).with_name("editor-source-import-oracle")/"Keys.cs";keys_bytes=keys.read_bytes();assert hashlib.sha256(keys_bytes).hexdigest()==KEYS_SHA
 hashes={}
 with tempfile.TemporaryDirectory(prefix="deadsync-real-preference-groups-") as folder:
  work=Path(folder)
  for name in ["Preferences"]+["Preferences"+g for g in GROUPS]:
   path="StepManiaEditor/Preferences/"+name+".cs";raw=original(args.source,path);(work/(name+".cs")).write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  path="StepManiaEditor/SubdivisionType.cs";raw=original(args.source,path);(work/"SubdivisionType.cs").write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  mini_path="StepManiaEditor/MiniMap.cs";raw=original(args.source,mini_path);hashes[mini_path]=hashlib.sha256(raw).hexdigest();mini=raw.decode("utf-8-sig")
  imgui_path="StepManiaEditor/ImGui/ImGuiUtils.cs";raw=original(args.source,imgui_path);hashes[imgui_path]=hashlib.sha256(raw).hexdigest();assert "public const int SceneWidgetPaddingDefaultDPI = 10;" in raw.decode("utf-8-sig")
  host=HOST.replace("ENUMS",member(mini,"Position")+member(mini,"SelectMode"));(work/"Host.cs").write_text(host);(work/"Keys.cs").write_bytes(keys_bytes);(work/"Program.cs").write_text(PROGRAM)
  (work/"Oracle.csproj").write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
  run=subprocess.run([str(args.dotnet),"build",str(work/"Oracle.csproj"),"-c","Release","--nologo","-v","q","-p:WarningLevel=0"],capture_output=True,text=True)
  if run.returncode:raise SystemExit(run.stdout+run.stderr)
  assembly=work/"bin/Release/net10.0/Oracle.dll"
  def execute(request):return json.loads(subprocess.run([str(args.dotnet),str(assembly)],input=json.dumps(request),capture_output=True,text=True,check=True).stdout)
  defaults=execute([dict(name="constructors-postload",raw="{}")])["cases"][0]["value"]
  payload=execute(requests(defaults))
  for case in payload["cases"]:
   case["accepted"]=case["name"]=="constructors-postload" or case["name"].endswith("-empty") or case["value"]["Player"]==3
   case["value"].pop("PreferencesKeyBinds",None)
  provenance=dict(editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sources=hashes,host_sha256=hashlib.sha256(host.encode()).hexdigest(),program_sha256=hashlib.sha256(PROGRAM.encode()).hexdigest(),keys_sha256=KEYS_SHA,assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),scope=__doc__)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(payload,indent=2)+"\n");args.output.with_suffix(".reference.json").write_text(json.dumps(provenance,indent=2)+"\n");print("Executed",len(payload["cases"]),"real-model constructor/converter/PostLoad cases")
if __name__=="__main__": main()
