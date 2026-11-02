#!/usr/bin/env python3
"""Execute pinned, unchanged GrooveAuthor preference groups and real Fumen.

Actual NoteColor model setters/PostLoad/cached getters, original MonoGame constructors and original uint helpers execute. Chartless Editor/ImGui/SavedSong UI hosts remain placeholders; no GPU texture draw claim.
"""
import argparse, hashlib, json, subprocess, tempfile
COLOR_SHA='bb40ac7bfc5b275965a9c7bdf14acf134df9756fbee1ba17f88181239b0e70ac'
MONOGAME_PIN='8e61b2effa9f8775bd63f24dc33b2f7924f9eba2'
from pathlib import Path
EDITOR_PIN="48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
LIBRARY_PIN="bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
KEYS_SHA="26d5351258a363590c9e51af83283a35aec7f5036cb03cd8313500506dd8d6a6"
HOST='using System;using System.Numerics;\nnamespace StepManiaEditor.AutogenConfig{static class PerformedChartConfigManager{public static Guid DefaultPerformedChartConfigGuid=Guid.Parse("6276c906-ea8f-43b3-9500-0ddeac7bdc22");}}\nnamespace Microsoft.Xna.Framework{public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}public struct Rectangle{public int X,Y,Width,Height;}}\nnamespace StepManiaEditor{\nclass Editor{public enum NoteEntryMode{Normal,AdvanceBySnap}public enum SpacingMode{ConstantTime,ConstantRow,Variable}public enum WaveFormScrollMode{CurrentTempo,CurrentTempoAndRate,MostCommonTempo}public bool GetChartAreaInChartSpaceWithoutHeader(out Microsoft.Xna.Framework.Rectangle r){r=new();return false;}}\nclass SavedSongInformation{public string FullPath;public void PostLoad(){}}\nstatic class ImGuiUtils{public const int SceneWidgetPaddingDefaultDPI=10;public static int UiScaled(int value)=>value;}\nstatic class Utils{public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;}\nclass MiniMap{ENUMS}\n}\n'
PROGRAM='using System.Text.Json;using System.Text.Json.Serialization;using System.Reflection;using StepManiaEditor;\nclass Program{\n static void Main(){var options=(JsonSerializerOptions)typeof(Preferences).GetField("SerializationOptions",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=Path.GetTempFileName();try{File.WriteAllText(path,raw);var p=Preferences.Load(new Editor(),path);var c=p.PreferencesNoteColor;var lookups=new List<object>();\n foreach(int player in new[]{-1,0,1,11})foreach(bool selected in new[]{false,true})foreach(bool held in new[]{false,true})foreach(int row in new[]{0,1,2,3,4,6,8,12,16,24,47}){\n var color=player<0?c.GetNoteColor(row,selected,held):c.GetNoteColor(player,row,selected,held);\n var hold=player<0?c.GetHoldBodyColor(selected,held):c.GetHoldBodyColor(player,selected,held);\n var roll=player<0?c.GetRollBodyColor(selected,held):c.GetRollBodyColor(player,selected,held);\n var mine=player<0?c.GetMineColor(selected,held):c.GetMineColor(player,selected,held);\n var ui=player<0?c.GetUINoteColor(row,selected):c.GetUINoteColor(player,row,selected);\n lookups.Add(new{player,selected,held,row,color=color.PackedValue,hold=hold.PackedValue,roll=roll.PackedValue,mine=mine.PackedValue,ui});}\n var pump=new List<object>();foreach(int player in new[]{-1,0,1,11})foreach(bool selected in new[]{false,true})foreach(bool held in new[]{false,true})foreach(int lane in new[]{0,1,2,3,4}){\n var color=player<0?c.GetPiuNoteColor(lane,selected,held):c.GetPiuNoteColor(player,lane,selected,held);\n var hold=player<0?c.GetPiuHoldBodyColor(lane,selected,held):c.GetPiuHoldBodyColor(player,lane,selected,held);\n var ui=player<0?c.GetPiuUINoteColor(lane,selected):c.GetPiuUINoteColor(player,lane,selected);\n pump.Add(new{player,selected,held,lane,color=color.PackedValue,hold=hold.PackedValue,ui});}\n output.Add(new{name=input.GetProperty("name").GetString(),raw,value=JsonSerializer.SerializeToElement(p,options),lookups,pump});}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{cases=output}));}}\n'
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
 result=[dict(name="defaults",raw=json.dumps({"Player":3}))]
 for key in ["SelectionColorMultiplier","HeldColorMultiplier","ArrowUIColorMultiplier","ArrowUISelectedColorMultiplier","RoutineNoteColorAlpha","PiuHoldSaturationMultiplier"]:
  for value in [-1,0,.251,12,1e20,-1e20,3.4028234663852886e38,-3.4028234663852886e38]:
   result.append(dict(name=key+"-"+str(value),raw=json.dumps({"Player":3,"PreferencesMultiplayer":{key:value}})))
 for i,value in enumerate([[-1,.251,2],[1e20,-1e20,.647],[3.4028234663852886e38,-3.4028234663852886e38,.99999994],[.251,.647,.419]]):
  for key in ["QuarterColor","Player0Color","PiuTopColor"]:
   result.append(dict(name=key+"-"+str(i),raw=json.dumps({"Player":3,"PreferencesMultiplayer":{key:dict(zip("XYZ",value))}})))
 return result

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ["source","reference-root","dotnet","output"]:parser.add_argument("--"+name,type=Path,required=True)
 parser.add_argument("--keys",type=Path,required=True)
 parser.add_argument("--monogame-color",type=Path,required=True)
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
  color_bytes=args.monogame_color.read_bytes();assert hashlib.sha256(color_bytes).hexdigest()==COLOR_SHA
  color=color_bytes.decode("utf-8-sig");constructors="\n".join(method(color,k)for k in ["public Color(float r, float g, float b, float alpha)","public Color(int r, int g, int b, int alpha)"])
  utils=original(args.source,"StepManiaEditor/Utils.cs");hashes["StepManiaEditor/Utils.cs"]=hashlib.sha256(utils).hexdigest();hashes["MonoGame.Framework/Color.cs@"+MONOGAME_PIN]=hashlib.sha256(color_bytes).hexdigest()
  actual_utils="\n".join(method(utils.decode("utf-8-sig"),k)for k in ["public static uint ToRGBA(float r, float g, float b, float a)","public static uint ColorRGBAMultiply(uint color, float multiplier)"])
  host=HOST.replace("ENUMS",member(mini,"Position")+member(mini,"SelectMode"))
  host=host.replace("public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}","public struct Color{uint _packedValue;public uint PackedValue=>_packedValue;public Color(float x,float y,float z):this(x,y,z,1){}public static Color White=>new();"+constructors+"}static class MathHelper{public static int Clamp(int v,int a,int b)=>Math.Clamp(v,a,b);}")
  host=host.replace("public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;",actual_utils)
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
   case["value"]={"PreferencesMultiplayer":case["value"]["PreferencesMultiplayer"]}
  assert all(case["accepted"] for case in payload["cases"])
  provenance=dict(model_cases=len(payload["cases"]),main_lookup_cases=sum(len(c["lookups"])for c in payload["cases"]),pump_lookup_cases=sum(len(c["pump"])for c in payload["cases"]),monogame_commit=MONOGAME_PIN,color_source_url="https://raw.githubusercontent.com/PerryAsleep/MonoGame/"+MONOGAME_PIN+"/MonoGame.Framework/Color.cs",editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sources=hashes,host_sha256=hashlib.sha256(host.encode()).hexdigest(),program_sha256=hashlib.sha256(PROGRAM.encode()).hexdigest(),keys_sha256=KEYS_SHA,assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),scope=__doc__)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(payload,indent=2)+"\n");args.output.with_suffix(".reference.json").write_text(json.dumps(provenance,indent=2)+"\n");print("Executed",len(payload["cases"]),"real NoteColor model/cache/MonoGame/uint consumer cases")
if __name__=="__main__": main()
