#!/usr/bin/env python3
"""Run pinned original UI factories, EventConfig defaults, collision and row methods.
Host adapters supply only chart identity, active BPM, event construction and rendering fields.
No Rust code or regenerated expected-value formula runs inside the source oracle.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
GA='48e18f71655d0f0d86144c63239331833501fda2';LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for directory,pin in [(a.editor_root,GA),(a.reference_root,LIB),(a.reference_root/'Fumen',FUMEN)]: assert subprocess.check_output(['git','-C',str(directory),'rev-parse','HEAD'],text=True).strip()==pin
sources={}
def source(path):
 raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/{path}']);sources[path]=hashlib.sha256(raw).hexdigest();return raw.decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);end=text.index('{',start)+1;depth=1
 while depth:
  depth+=(text[end]=='{')-(text[end]=='}');end+=1
 return text[start:end]
ui=source('UI/UIEditEvents.cs');cfg=source('EditorEvents/EventConfig.cs');pos=source('EditorPosition.cs');pattern=source('EditorEvents/EditorPatternEvent.cs');chart=source('EditorChart.cs')
ui_names=['Tempo','InterpolatedScrollRate','ScrollRate','Stop','Delay','Warp','FakeRegion','Ticks','ComboMultipliers','TimeSignature','Label','Attack','Pattern']
config_names=['Stop','Delay','Warp','Fake','TickCount','Multipliers','TimeSignature','Label','Tempo','ScrollRate','ScrollRateInterpolation','Attack','Pattern']
ui_methods='\n'.join(member(ui,'private EditorEvent Create'+name+'Event(').replace('private EditorEvent','public EditorEvent',1) for name in ui_names)
config_methods='\n'.join(member(cfg,'public static EventConfig Create'+name+'Config(') for name in config_names)+'\n'+member(cfg,'public static Modifier CreateDefaultModifier(')
constants='\n'.join(line.strip() for line in chart.splitlines() if any(' '+name+' ' in line for name in ['DefaultTempo','DefaultTimeSignature','DefaultScrollRate','DefaultTickCount','DefaultHitMultiplier','DefaultMissMultiplier']) and ('public const ' in line or 'public static readonly' in line))
profile=[]
for path,name in [('AutogenConfig/PatternConfigManager.cs','DefaultPatternConfigSixteenthsGuid'),('AutogenConfig/PerformedChartConfigManager.cs','DefaultPerformedChartPatternBalancedGuid')]:
 text=source(path);profile.append(next(line.strip() for line in text.splitlines() if name+' =' in line))
# Execute the original constructor, including Random.Next; render WidthDirty is inert.
constructor=member(pattern,'public EditorPatternEvent(EventConfig config) : base(config)')
# The Definition shape is a projection host; defaults come from the unchanged constructor.
code='''using System.Text.Json;using StepManiaLibrary;using Fumen;using Fumen.ChartDefinition;using Fumen.Converters;using static Fumen.Converters.SMCommon;
class EditorChart { CONSTANTS public int GetMeasureForNewTimeSignatureAtRow(int row)=>row/192; }
class Editor {public EditorChart GetFocusedChart()=>new();}
class EditorRateAlteringEvent(double bpm) {public double GetTempo()=>bpm; public double GetSecondsPerRow()=>60/bpm/48;}
class EventConfig { public Event Value;public EventConfig(EditorChart c,Event e){Value=e;} CONFIG_METHODS }
class EditorEvent { public Event Value; public EditorEvent(EventConfig c){Value=c.Value;}public static EditorEvent CreateEvent(EventConfig c)=>c.Value is Pattern?new EditorPatternEvent(c):new EditorEvent(c); }
class PatternConfigManager { PROFILE0 }
class PerformedChartConfigManager { PROFILE1 }
class Definition { public Guid PatternConfigGuid{get;set;} public Guid PerformedChartConfigGuid{get;set;}public int Length{get;set;}public int RandomSeed{get;set;}public bool StartPositionInclusive{get;set;}public bool EndPositionInclusive{get;set;}public bool IgnorePrecedingDistribution{get;set;} }
class EditorPatternEvent:EditorEvent { public Definition EventDefinition;public bool WidthDirty; CONSTRUCTOR }
class EditorPosition { public double ChartPosition; NEAREST }
class UIEditEvents {Editor Editor=new(); UI_METHODS COLLISION public static bool Can(List<EditorEvent> events,Type t)=>CanTypeOfEventExistAtRow(events,t); }
class Program {static object Project(EditorEvent e)=>new {Type=e.Value.GetType().Name,Row=e.Value.IntegerPosition,Fields=JsonSerializer.SerializeToElement(e.Value,e.Value.GetType(),new JsonSerializerOptions{IncludeFields=true}),Pattern=e is EditorPatternEvent p?p.EventDefinition:null};
static void Main(){var ui=new UIEditEvents();var methods=typeof(UIEditEvents).GetMethods().Where(m=>m.Name.StartsWith("Create")&&m.ReturnType==typeof(EditorEvent)).OrderBy(m=>m.Name);var cases=new List<object>();foreach(var bpm in new[]{60.0,120.0,180.0,333.333})foreach(var m in methods){var args=m.GetParameters().Length==2?new object[]{192,new EditorRateAlteringEvent(bpm)}:new object[]{192};var e=(EditorEvent)m.Invoke(ui,args);cases.Add(new{Method=m.Name,Bpm=bpm,Event=Project(e)});}var positions=new[]{-50.5,-0.1,0.0,0.5,1.5,1.5000001,47.5,47.6}.Select(v=>{var p=new EditorPosition{ChartPosition=v};return new{Position=v,Row=Math.Max(0,p.GetNearestRow())};});var basic=new EditorEvent(new EventConfig(new(),new Tempo(120)));var pat=new EditorPatternEvent(EventConfig.CreatePatternConfig(new(),192));Console.Write(JsonSerializer.Serialize(new{Cases=cases,Positions=positions,Collision=new {Empty=UIEditEvents.Can(new(),typeof(EditorEvent)),Same=UIEditEvents.Can(new(){basic},typeof(EditorEvent)),Different=UIEditEvents.Can(new(){basic},typeof(EditorPatternEvent))},Seeds=Enumerable.Range(0,16).Select(_=>new EditorPatternEvent(EventConfig.CreatePatternConfig(new(),192)).EventDefinition.RandomSeed)}));}}
'''
for key,value in {'CONSTANTS':constants,'CONFIG_METHODS':config_methods,'PROFILE0':profile[0],'PROFILE1':profile[1],'CONSTRUCTOR':constructor,'NEAREST':member(pos[pos.rindex('public int GetNearestRow()'):],'public int GetNearestRow()'),'UI_METHODS':ui_methods,'COLLISION':member(ui,'private static bool CanTypeOfEventExistAtRow(')}.items():code=code.replace(key,value)
with tempfile.TemporaryDirectory(prefix='deadsync-quick-add-source-') as folder:
 root=Path(folder);(root/'Program.cs').write_text(code)
 (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{a.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference><Reference Include="StepManiaLibrary"><HintPath>{a.reference_root}/StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
 result=subprocess.run([str(a.dotnet),str(root/'bin/Release/net10.0/Oracle.dll')],text=True,capture_output=True,check=True)
 output=json.loads(result.stdout)
a.output.parent.mkdir(parents=True,exist_ok=True)
# Random values intentionally are witnesses, not deterministic golden seed values.
a.output.write_text(json.dumps(output,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':GA,'LibraryCommit':LIB,'FumenCommit':FUMEN,'SourcesSha256':sources,'AdapterSha256':hashlib.sha256(code.encode()).hexdigest(),'Scope':'Unchanged UI factories, EventConfig factories, nearest row, exact-type collision predicate, and original pattern constructor; Fumen concrete event models; chart ownership/tempo/render adapter only. No writer roundtrip claim.'},indent=2)+'\n')
print(f'Executed {len(output["Cases"])} source factory cases, {len(output["Positions"])} row cases and source collision predicate.')
