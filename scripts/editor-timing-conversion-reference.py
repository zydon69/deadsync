#!/usr/bin/env python3
"""Execute original Warp/negative-stop actions with unchanged original editor timing passes."""
import argparse,base64,hashlib,json,re,subprocess,tempfile
from pathlib import Path
LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.reference_root/'Fumen'
for folder,pin in [(a.reference_root,LIB),(root,PIN)]:
 assert subprocess.check_output(['git','-C',str(folder),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(folder),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
GA='48e18f71655d0f0d86144c63239331833501fda2'
assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==GA
ga_paths=['StepManiaEditor/Editor.cs','StepManiaEditor/EditorSong.cs','StepManiaEditor/EditorChart.cs','StepManiaEditor/Preferences/Preferences.cs']
ga_blobs={path:subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:{path}'])for path in ga_paths}
paths=['Fumen/Converters/SMCommon.cs','Fumen/Converters/SMPropertyParsers.cs','Fumen/Converters/SMReader.cs','Fumen/Converters/SSCReader.cs','Fumen/Converters/MSDFile.cs','Fumen/Converters/SMWriter.cs','Fumen/Converters/SSCWriter.cs','Fumen/Converters/SMWriterBase.cs'];blobs={path:subprocess.check_output(['git','-C',str(root),'show',f'{PIN}:{path}'])for path in paths}
import runpy
repo=Path(__file__).resolve().parent.parent
module=runpy.run_path(str(repo/'scripts/editor-generate-fixtures.py'));timing_hash=module['prepare_timing_reference'](a.editor_root)
def original(path):return subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/{path}'],text=True,encoding='utf-8-sig')
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);depth=1;end=brace+1
 while depth:
  if text[end]=='{':depth+=1
  if text[end]=='}':depth-=1
  end+=1
 return text[start:end]
chart=original('EditorChart.cs');warp=original('EditorEvents/EditorWarpEvent.cs')
base=(repo/'scripts/editor-oracle/obj/EditorTimingReference.cs').read_text();base=base[:base.index('internal class DensityReference')].replace('namespace EditorTimingOracle;','namespace StepManiaEditor;')
base=base.replace('public static EditorEvent CreateEvent(EventConfig config)=>new EditorAttackEvent(config.Chart,config.Attack,[]);','public static EditorEvent CreateEvent(EventConfig config)=>config.ConversionEvent??new EditorAttackEvent(config.Chart,config.Attack,[]);')
base=base.replace('public virtual double GetEndChartPosition() => GetRow();','public virtual double GetEndChartPosition() => GetRow();\n    public int GetEndRow() => (int)GetEndChartPosition();',1)
base=base.replace('public Attack Attack=attack;','public Attack Attack=attack;\n public EditorEvent? ConversionEvent;\n public static EventConfig CreateStopConfig(EditorChart chart,int row,double seconds)=>new(chart,new Attack()){ConversionEvent=new EditorStopEvent(chart,new Stop(seconds){IntegerPosition=row})};\n public static EventConfig CreateWarpConfig(EditorChart chart,int row,int length)=>new(chart,new Attack()){ConversionEvent=new EditorWarpEvent(chart,new Warp(Math.Max(1,length)){IntegerPosition=row})};',1)
base=base.replace('public EditorRateAlteringEvent ForEvent(Event value)', 'public EditorRateAlteringEvent FindActiveRateAlteringEventForTime(double time,bool equal) => this.LastOrDefault(rate=>equal?rate.GetChartTime()<=time:rate.GetChartTime()<time)??this.FirstOrDefault()!;\n    public EditorRateAlteringEvent ForEvent(Event value)',1)
base=base.replace('public void DeleteEvents(IEnumerable<EditorEvent> values) {var set=values.ToHashSet();EditorEvents.RemoveAll(set.Contains);}','public void DeleteEvents(IEnumerable<EditorEvent> values) {var set=values.ToHashSet();EditorEvents.RemoveAll(set.Contains);Refresh();}',1)
base=base.replace('public void AddEvents(IEnumerable<EditorEvent> values) {EditorEvents.AddRange(values.Where(value=>!EditorEvents.Contains(value)).Distinct());EditorEvents.Sort((a,b)=>new SMEventComparer(EventOrder.Order).Compare(a.Data,b.Data));}','public void AddEvents(IEnumerable<EditorEvent> values) {EditorEvents.AddRange(values.Where(value=>!EditorEvents.Contains(value)).Distinct());Refresh();}',1)
base=base.replace('public void Refresh() {EditorEvents.Sort','public void Refresh() {RateAlteringEvents.Clear();Holds.Clear();EditorEvents.Sort',1)
base=base.replace('private const int NotificationTimingChanged=0;', 'public IEnumerable<EditorWarpEvent> GetWarps()=>EditorEvents.OfType<EditorWarpEvent>();\npublic IEnumerable<EditorStopEvent> GetStops()=>EditorEvents.OfType<EditorStopEvent>();\npublic void AddEvent(EditorEvent value)=>AddEvents([value]);\n'+member(chart,'public bool TryGetChartPositionFromTime(')+'\nprivate const int NotificationTimingChanged=0;',1)
base+='\ninternal class Editor {public object[] Selected=[];public void OnNoteTransformationBegin(){}public void OnNoteTransformationEnd(List<EditorEvent> values){Selected=values.Select(Project).ToArray();}public static object Project(EditorEvent e)=>new{Kind=e switch {EditorWarpEvent=>"Warp",EditorStopEvent=>"Stop",EditorDelayEvent=>"Delay",EditorTempoEvent=>"Tempo",EditorTimeSignatureEvent=>"Signature",_=>"Other"},Row=e.GetRow(),Value=e switch{EditorWarpEvent w=>(double)w.GetWarpLengthRows(),EditorStopEvent s=>s.GetStopLengthSeconds(),EditorDelayEvent d=>d.GetDelayLengthSeconds(),EditorTempoEvent t=>t.GetTempo(),_=>0},Time=e.GetChartTime()};}\n'
actions={name:original('EditorActions/'+name+'.cs')for name in ['ActionChangeWarpsToNegativeStops','ActionChangeNegativeStopsToWarps']}
def event(kind,row,value=0,selected=True):return dict(Kind=kind,Row=row,Value=value,Selected=selected)
cases=[]
def add(name,direction,events,all=True):cases.append(dict(Name=name,Direction=direction,All=all,Events=[event('Signature',0),event('Tempo',0,60)]+events))
add('warp-none','WarpToStop',[])
add('warp-simple','WarpToStop',[event('Warp',192,48)])
add('warp-stop-collision-deletes-original','WarpToStop',[event('Warp',192,48),event('Stop',192,4)])
add('warp-overlap','WarpToStop',[event('Warp',48,96),event('Warp',96,96)])
add('warp-tempo-inside','WarpToStop',[event('Warp',48,144),event('Tempo',96,120)])
for row in [48,72,96]:add('warp-delay-'+str(row),'WarpToStop',[event('Warp',48,48),event('Delay',row,.25)])
add('warp-selected-inside-unselected','WarpToStop',[event('Warp',48,144,False),event('Warp',96,48)],False)
add('warp-selected-reversed','WarpToStop',[event('Warp',96,96),event('Warp',48,96)],False)
add('stop-none','StopToWarp',[])
add('stop-positive-unchanged','StopToWarp',[event('Stop',192,1)])
add('stop-simple','StopToWarp',[event('Stop',192,-1)])
add('stop-warp-collision-deletes-original','StopToWarp',[event('Stop',192,-1),event('Warp',192,48)])
add('stop-overlap','StopToWarp',[event('Stop',48,-2),event('Stop',96,-1)])
add('stop-tempo-inside','StopToWarp',[event('Stop',48,-2),event('Tempo',96,120)])
for row in [48,72,96]:add('stop-delay-'+str(row),'StopToWarp',[event('Stop',48,-1),event('Delay',row,.25)])
add('stop-tiny-clamps-one','StopToWarp',[event('Stop',48,-.0001)])
add('stop-half-row-even','StopToWarp',[event('Stop',48,-.5/48)])
add('stop-one-half-even','StopToWarp',[event('Stop',49,-1.5/48)])
add('stop-selected-reversed','StopToWarp',[event('Stop',96,-1),event('Stop',48,-2)],False)
request=dict(Cases=cases)
project=repo/'scripts/editor-timing-conversion-oracle'
with tempfile.TemporaryDirectory(prefix='deadsync-timing-conversion-build-')as directory:
 folder=Path(directory);(folder/'Adapter.cs').write_text(base)
 for name,text in actions.items():(folder/(name+'.cs')).write_text(text)
 (folder/'Program.cs').write_bytes((project/'Program.cs').read_bytes());(folder/'request.json').write_text(json.dumps(request));(folder/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{root}/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference><Reference Include="StepManiaLibrary"><HintPath>{a.reference_root}/StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(folder/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=folder/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly),str(folder/'request.json')],capture_output=True,text=True);assert output.returncode==0,output.stderr;expected=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');sha=lambda b:hashlib.sha256(b).hexdigest()
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=GA,library_commit=LIB,fumen_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Two complete unchanged original actions and original RefreshEventTimingData/RefreshRateAlteringEvents plus row/time mapping bodies reused from the existing original timing oracle. Explicit storage/tree-search/factory and inert-render adapters; warp factory mirrors original constructor minimum-one clamp. No full GUI, original writer or file format roundtrip executed.',timing_methods_sha256=timing_hash,original_actions_sha256={name:sha(text.encode())for name,text in actions.items()},original_chart_sha256=sha(chart.encode()),original_warp_sha256=sha(warp.encode()),adapter_sha256=sha(base.encode()),program_sha256=sha((project/'Program.cs').read_bytes()),assembly_sha256=assembly_hash,request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} original timing conversion actions')
