#!/usr/bin/env python3
"""Execute original Select All bodies and Selection with explicit event/tree adapters."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
PIN='48e18f71655d0f0d86144c63239331833501fda2'
assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==PIN
blobs={}
def source(path):
 data=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{PIN}:StepManiaEditor/{path}']);blobs[path]=data;return data.decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);end=brace+1;depth=1
 while depth:
  if text[end]=='{':depth+=1
  if text[end]=='}':depth-=1
  end+=1
 return text[start:end]
active=source('ActiveEditorChart.cs');selection=source('Selection.cs');ui=source('UI/UIEditEvents.cs')
methods=[member(active,sig)for sig in ['public void OnSelectAll()','public void OnSelectAll(int player)','public void OnSelectAllAlt()','public void OnSelectAllShift()','public void OnSelectAll(Func<EditorEvent, bool> isSelectable)']]
classes=[]
for name in ['TapNote','MineNote','FakeNote','LiftNote','HoldNote','Pattern','Tempo','Stop','Attack']:
 text=source('EditorEvents/Editor'+name+'Event.cs');cls='Editor'+name+'Event';members=[member(text,'public override bool '+sig+'()')for sig in ['IsSelectableWithoutModifiers','IsSelectableWithModifiers']]
 classes.append('internal class '+cls+':EditorEvent{'+''.join(members)+('public bool IsRoll()=>Kind=="Roll";'if name=='HoldNote'else'')+'}')
predicates={
'Taps':'e => e is EditorTapNoteEvent','Mines':'e => e is EditorMineNoteEvent','Fakes':'e => e is EditorFakeNoteEvent','Lifts':'e => e is EditorLiftNoteEvent','Holds':'e => e is EditorHoldNoteEvent hn && !hn.IsRoll()','Rolls':'e => e is EditorHoldNoteEvent hn && hn.IsRoll()','HoldsAndRolls':'e => e is EditorHoldNoteEvent'}
for predicate in predicates.values():assert predicate in ui
cases=[]
for preset in ['Notes',*predicates,'Miscellaneous','NotesAndMiscellaneous','Patterns']:
 for owner in ([None,0,1]if preset in ['Notes',*predicates]else[None]):
  for initial in [False,True]:
   events=[dict(Kind=k,Player=p,Id=f'{k}-{p}',Selected=False)for p in [0,1]for k in ['Tap','Mine','Fake','Lift','Hold','Roll']]+[dict(Kind=k,Player=0,Id=k,Selected=initial and k=='Tempo')for k in ['Tempo','Stop','Pattern','Attack']]
   cases.append(dict(Name=f'{preset}-{owner}-{initial}',Preset=preset,Player=owner,Events=events))
request={'Cases':cases}
adapter='''using System;using System.Linq;using System.Collections.Generic;
namespace StepManiaEditor {
internal class EditorEvent {public string Kind,Id;public int Player;private bool selected;public virtual bool IsSelectableWithoutModifiers()=>false;public virtual bool IsSelectableWithModifiers()=>false;public int GetPlayer()=>Player;public bool IsSelected()=>selected;public void Select()=>selected=true;public void Deselect()=>selected=false;public bool IsAddedToChart()=>true;}
'''+''.join(classes)+'''internal class ChartAdapter {public List<EditorEvent> Events=[];public IEnumerable<EditorEvent> GetEvents()=>Events;}
internal class ActiveEditorChart {public ChartAdapter Chart=new();public Selection Selection=new();'''+''.join(methods)+'}}'
program='''using System.Text.Json;using StepManiaEditor;
var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var results=new List<object>();
foreach(var item in request.GetProperty("Cases").EnumerateArray()){
var chart=new ActiveEditorChart();foreach(var value in item.GetProperty("Events").EnumerateArray()){
EditorEvent e=value.GetProperty("Kind").GetString() switch {"Tap"=>new EditorTapNoteEvent(),"Mine"=>new EditorMineNoteEvent(),"Fake"=>new EditorFakeNoteEvent(),"Lift"=>new EditorLiftNoteEvent(),"Hold" or "Roll"=>new EditorHoldNoteEvent(),"Tempo"=>new EditorTempoEvent(),"Stop"=>new EditorStopEvent(),"Pattern"=>new EditorPatternEvent(),"Attack"=>new EditorAttackEvent(),_=>throw new Exception()};
e.Id=value.GetProperty("Id").GetString();e.Kind=value.GetProperty("Kind").GetString();e.Player=value.GetProperty("Player").GetInt32();chart.Chart.Events.Add(e);if(value.GetProperty("Selected").GetBoolean())chart.Selection.SelectEvent(e,true);}
int? player=item.GetProperty("Player").ValueKind==JsonValueKind.Null?null:item.GetProperty("Player").GetInt32();
switch(item.GetProperty("Preset").GetString()){
case "Notes":if(player.HasValue)chart.OnSelectAll(player.Value);else chart.OnSelectAll();break;
case "Miscellaneous":chart.OnSelectAllAlt();break;case "NotesAndMiscellaneous":chart.OnSelectAllShift();break;
case "Patterns":chart.OnSelectAll(e=>e is EditorPatternEvent);break;
'''+''.join('case "'+name+'":chart.OnSelectAll(e => ('+predicate.removeprefix('e => ') +') && (!player.HasValue || e.GetPlayer()==player.Value));break;\n'for name,predicate in predicates.items())+'''}
results.Add(new{Name=item.GetProperty("Name").GetString(),Selected=chart.Selection.GetSelectedEvents().Select(e=>e.Id).Order().ToArray(),Last=chart.Selection.GetLastSelectedEvent()?.Id});}
Console.Write(JsonSerializer.Serialize(new{Cases=results}));'''
sha=lambda b:hashlib.sha256(b).hexdigest()
with tempfile.TemporaryDirectory(prefix='deadsync-selection-presets-')as d:
 root=Path(d);(root/'Adapter.cs').write_text(adapter);(root/'Selection.cs').write_text(selection);(root/'Program.cs').write_text(program);(root/'request.json').write_text(json.dumps(request));(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
 assembly=root/'bin/Release/net10.0/Oracle.dll';run=subprocess.run([str(a.dotnet),str(assembly),str(root/'request.json')],capture_output=True,text=True,check=True);expected=json.loads(run.stdout);assembly_hash=sha(assembly.read_bytes())
a.output.parent.mkdir(parents=True,exist_ok=True)
rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n')
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Five unchanged ActiveEditorChart SelectAll methods, complete original Selection, original IsSelectable methods of nine event classes. Explicit inert event/tree adapters; original menu predicates checked against UI text. This proves additive membership and last-selection behavior, not GUI or Fumen serialization.',source_sha256={n:sha(b)for n,b in blobs.items()},method_sha256=[sha(s.encode())for s in methods],adapter_sha256=sha(adapter.encode()),program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n')
print(f'Wrote {len(cases)} original selection preset cases')
