#!/usr/bin/env python3
"""Execute pinned, unchanged GrooveAuthor preference groups and real Fumen.

Original Editor.UpdateDensityGraphBounds and pinned MonoGame constructors execute unchanged. Actual Fumen packed-color interpolation executes. Only external UI/cache dependencies are hosted: controlled chart/focus anchors, selectable positive ImGui DPI,
SavedSong placeholder, and GPU Color/uint color cache plumbing. Their pixels,
recent-song behavior and runtime settings are not asserted by this oracle.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
EDITOR_PIN="48e18f71655d0f0d86144c63239331833501fda2"
FUMEN_PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
LIBRARY_PIN="bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
MONOGAME_PIN="8e61b2effa9f8775bd63f24dc33b2f7924f9eba2"
COLOR_SHA="bb40ac7bfc5b275965a9c7bdf14acf134df9756fbee1ba17f88181239b0e70ac"
KEYS_SHA="26d5351258a363590c9e51af83283a35aec7f5036cb03cd8313500506dd8d6a6"
HOST='using System;using System.Numerics;\nnamespace StepManiaEditor.AutogenConfig{static class PerformedChartConfigManager{public static Guid DefaultPerformedChartConfigGuid=Guid.Parse("6276c906-ea8f-43b3-9500-0ddeac7bdc22");}}\nnamespace Microsoft.Xna.Framework{public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}public struct Rectangle{public int X,Y,Width,Height;}}\nnamespace StepManiaEditor{\nclass Editor{public enum NoteEntryMode{Normal,AdvanceBySnap}public enum SpacingMode{ConstantTime,ConstantRow,Variable}public enum WaveFormScrollMode{CurrentTempo,CurrentTempoAndRate,MostCommonTempo}public bool GetChartAreaInChartSpaceWithoutHeader(out Microsoft.Xna.Framework.Rectangle r){r=new();return false;}}\nclass SavedSongInformation{public string FullPath;public void PostLoad(){}}\nstatic class ImGuiUtils{public const int SceneWidgetPaddingDefaultDPI=10;public static int UiScaled(int value)=>value;}\nstatic class Utils{public static uint ToRGBA(float r,float g,float b,float a)=>0;public static uint ColorRGBAMultiply(uint c,float m)=>0;}\nclass MiniMap{ENUMS}\n}\n'
PROGRAM='using System.Text.Json;using System.Reflection;using StepManiaEditor;using Fumen;\nclass Program {\n static object Color(uint packed)=>new[]{(packed&255),((packed>>8)&255),((packed>>16)&255),((packed>>24)&255)};\n static void Main(){var options=(JsonSerializerOptions)typeof(Preferences).GetField("SerializationOptions",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){\n var raw=input.GetProperty("raw").GetString(); var path=System.IO.Path.GetTempFileName();try{\n var editor=new Editor(); File.WriteAllText(path,raw);var p=Preferences.Load(editor,path);var d=p.PreferencesDensityGraph;\n var low=new Microsoft.Xna.Framework.Color(d.DensityGraphLowColor);var high=new Microsoft.Xna.Framework.Color(d.DensityGraphHighColor);var bg=new Microsoft.Xna.Framework.Color(d.DensityGraphBackgroundColor);\n var colors=new List<object>();foreach(float t in new float[]{0,.25f,.5f,.75f,1})colors.Add(new{t,bytes=Color(Fumen.ColorUtils.ColorRGBAInterpolate(low.PackedValue,high.PackedValue,t))});\n var bounds=new List<object>();foreach(double dpi in new double[]{1,1.5,8})foreach(var pos in Enum.GetValues<PreferencesDensityGraph.DensityGraphPosition>())foreach(bool focused in new[]{false,true}){d.DensityGraphPositionValue=pos;bounds.Add(editor.RunBounds(dpi,focused));}\n d.DensityGraphPositionValue=PreferencesDensityGraph.DefaultDensityGraphPositionValue;\n output.Add(new{name=input.GetProperty("name").GetString(),raw,value=JsonSerializer.SerializeToElement(p,options),bounds,low=Color(low.PackedValue),high=Color(high.PackedValue),background=Color(bg.PackedValue),colors});\n }finally{File.Delete(path);}}\n Console.Write(JsonSerializer.Serialize(new{cases=output}));}\n}'
GROUPS=["Audio","WaveForm","Dark","Scroll","Selection","MiniMap","Options","Receptors","Performance","Stream","DensityGraph","NoteColor","KeyBinds"]
def git(root,*args): return subprocess.check_output(["git","-C",str(root),*args])
def original(root,path): return git(root,"show",EDITOR_PIN+":"+path)
def member(text,name):
 start=text.index("public enum "+name); begin=text.index("{",start); end=text.index("}",begin); return text[start:end+1]
def requests(defaults):
 result=[]
 for h,o,w in [(90,10,-10),(0,0,0),(1,10,-10),(2,10,-10),(-1,-2048,2048),(2048,2048,-2048),(2147483647,2147483647,2147483647),(-2147483648,-2147483648,-2147483648)]:
  d={"DensityGraphHeight":h,"DensityGraphPositionOffsets":{},"DensityGraphWidthOffsets":{}}
  for name in ["RightSideOfWindow","LeftSideOfWindow","FocusedChartWithoutScaling","FocusedChartWithScaling","TopOfFocusedChart","BottomOfFocusedChart"]:d["DensityGraphPositionOffsets"][name]=o;d["DensityGraphWidthOffsets"][name]=w
  result.append(dict(name="geometry-"+str(h),raw=json.dumps({"Player":3,"PreferencesDensityGraph":d})))
 for i,(lo,hi,bg) in enumerate([([-2,.251,2,.5],[1,.705,.282,2],[.118,.118,.118,1]),([3.4028234663852886e38,-3.4028234663852886e38,.99999994,0],[0,.5,.25,1],[2,-1,.1,.5]),([1e20,-1e20,.25,.5],[-1e20,1e20,.75,.25],[1e20,-1e20,.1,1]),([.251,.647,.419,1],[.705,.282,.282,1],[.118,.118,.118,1])]):
  d={key:dict(zip("XYZW",v))for key,v in [("DensityGraphLowColor",lo),("DensityGraphHighColor",hi),("DensityGraphBackgroundColor",bg)]}
  result.append(dict(name="palette-"+str(i),raw=json.dumps({"Player":3,"PreferencesDensityGraph":d})))
 return result

def method(text, signature):
 start=text.index(signature);brace=text.index("{",start);depth=1;end=brace+1
 while depth:
  depth += (text[end]=="{")-(text[end]=="}");end+=1
 return text[start:end]

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ["source","reference-root","dotnet","output"]:parser.add_argument("--"+name,type=Path,required=True)
 parser.add_argument("--keys",type=Path)
 parser.add_argument("--monogame-color",type=Path,required=True,help="Original MonoGame.Framework/Color.cs pinned by GrooveAuthor submodule; exact bytes SHA256 verified")
 args=parser.parse_args()
 assert git(args.source,"rev-parse","HEAD").decode().strip()==EDITOR_PIN
 assert git(args.reference_root,"rev-parse","HEAD").decode().strip()==LIBRARY_PIN
 fumen=args.reference_root/"Fumen";assert git(fumen,"rev-parse","HEAD").decode().strip()==FUMEN_PIN
 dll=fumen/"Fumen/bin/Release/net10.0/Fumen.dll"
 if not dll.exists(): raise SystemExit("Build the pinned Fumen net10.0 Release assembly first")
 keys=args.keys or Path(__file__).with_name("editor-source-import-oracle")/"Keys.cs";keys_bytes=keys.read_bytes();assert hashlib.sha256(keys_bytes).hexdigest()==KEYS_SHA
 color_bytes=args.monogame_color.read_bytes();assert hashlib.sha256(color_bytes).hexdigest()==COLOR_SHA
 color=color_bytes.decode("utf-8-sig")
 constructors="\n".join(method(color,signature) for signature in ["public Color(Vector4 color)","public Color(float r, float g, float b, float alpha)","public Color(int r, int g, int b, int alpha)"])
 editor_bytes=original(args.source,"StepManiaEditor/Editor.cs");bounds_method=method(editor_bytes.decode("utf-8-sig"),"private void UpdateDensityGraphBounds()")
 hashes={"MonoGame.Framework/Color.cs@"+MONOGAME_PIN:hashlib.sha256(color_bytes).hexdigest(),"StepManiaEditor/Editor.cs":hashlib.sha256(editor_bytes).hexdigest()}
 with tempfile.TemporaryDirectory(prefix="deadsync-real-preference-groups-") as folder:
  work=Path(folder)
  for name in ["Preferences"]+["Preferences"+g for g in GROUPS]:
   path="StepManiaEditor/Preferences/"+name+".cs";raw=original(args.source,path);(work/(name+".cs")).write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  path="StepManiaEditor/SubdivisionType.cs";raw=original(args.source,path);(work/"SubdivisionType.cs").write_bytes(raw);hashes[path]=hashlib.sha256(raw).hexdigest()
  mini_path="StepManiaEditor/MiniMap.cs";raw=original(args.source,mini_path);hashes[mini_path]=hashlib.sha256(raw).hexdigest();mini=raw.decode("utf-8-sig")
  imgui_path="StepManiaEditor/ImGui/ImGuiUtils.cs";raw=original(args.source,imgui_path);hashes[imgui_path]=hashlib.sha256(raw).hexdigest();assert "public const int SceneWidgetPaddingDefaultDPI = 10;" in raw.decode("utf-8-sig")
  host=("using Rectangle=Microsoft.Xna.Framework.Rectangle;\n"+HOST).replace("ENUMS",member(mini,"Position")+member(mini,"SelectMode"))
  host=host.replace("class Editor{", "partial class Editor{").replace("public static int UiScaled(int value)=>value;", "public static double Dpi=1;public static double GetDpiScale()=>Dpi;"+method(raw.decode("utf-8-sig"),"public static int UiScaled(int value)"))
  host=host.replace("public struct Color{public Color(float x,float y,float z){}public static Color White=>new();}","public struct Color{private uint _packedValue;public uint PackedValue=>_packedValue;public Color(float x,float y,float z):this(x,y,z,1){}public static Color White=>new();"+constructors+"}static class MathHelper{public static int Clamp(int v,int min,int max)=>Math.Clamp(v,min,max);}")
  host=host.replace("public int X,Y,Width,Height;", "public int X,Y,Width,Height;public Rectangle(int x,int y,int w,int h){X=x;Y=y;Width=w;Height=h;}")
  host+="\nnamespace StepManiaEditor {class StepDensityEffect{public enum Orientation{Vertical,Horizontal}public Microsoft.Xna.Framework.Rectangle Rect;public Orientation Mode;public void UpdateBounds(Microsoft.Xna.Framework.Rectangle r,Orientation mode){Rect=r;Mode=mode;}}class FocusHost{public int GetScreenSpaceXOfMiscEventsEnd()=>600;public int GetScreenSpaceXOfMiscEventsEndWithCurrentScale()=>650;public int GetScreenSpaceXOfLanesStart()=>100;public int GetScreenSpaceXOfLanesEnd()=>500;}partial class Editor{Microsoft.Xna.Framework.Rectangle ChartArea=new(10,20,1280,720);StepDensityEffect DensityGraph=new();bool Focused;int GetChartHeaderHeight()=>40;FocusHost GetFocusedChartData()=>Focused?new():null;public object RunBounds(double dpi,bool focused){ImGuiUtils.Dpi=dpi;Focused=focused;UpdateDensityGraphBounds();var r=DensityGraph.Rect;return new{dpi,focused,position=Preferences.Instance.PreferencesDensityGraph.DensityGraphPositionValue.ToString(),x=r.X,y=r.Y,width=r.Width,height=r.Height,orientation=DensityGraph.Mode.ToString()};}"+bounds_method+"}}"
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
   case["value"]={"PreferencesDensityGraph":case["value"]["PreferencesDensityGraph"]}
  provenance=dict(monogame_commit=MONOGAME_PIN,color_source_url="https://raw.githubusercontent.com/PerryAsleep/MonoGame/"+MONOGAME_PIN+"/MonoGame.Framework/Color.cs",geometry_cases=sum(len(c["bounds"])for c in payload["cases"]),editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sources=hashes,host_sha256=hashlib.sha256(host.encode()).hexdigest(),program_sha256=hashlib.sha256(PROGRAM.encode()).hexdigest(),keys_sha256=KEYS_SHA,assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),fumen_assembly_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),scope=__doc__)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(payload["cases"],indent=2)+"\n");args.output.with_suffix(".reference.json").write_text(json.dumps(provenance,indent=2)+"\n");print("Executed",len(payload["cases"]),"original density models + bounds + MonoGame constructors/Fumen interpolation cases")
if __name__=="__main__": main()
