#!/usr/bin/env python3
"""Original Selection and selected timing callbacks/constructors routing oracle."""
import argparse,subprocess,tempfile,json,hashlib,re
from pathlib import Path
p=argparse.ArgumentParser()
for k in ['editor-root','dotnet','output']:p.add_argument('--'+k,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==pin
sources={}
def source(n):
 text=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/'+n],text=True,encoding='utf-8-sig');sources[n]=text;return text
def member(text,sig):
 start=text.index(sig);i=text.index('{',start);j=i+1;depth=1
 while depth:
  if text[j]=='{':depth+=1
  if text[j]=='}':depth-=1
  j+=1
 return text[start:j]
selection=source('Selection.cs');ui=source('UI/UIEditEvents.cs')
methods='\n'.join(member(ui,n) for n in ['private bool TryGetFocusedChartSelection','private void ConvertWarpsToNegativeStops','private void ConvertNegativeStopsToWarps','public void ConvertSelectedWarpsToNegativeStops','public void ConvertSelectedNegativeStopsToWarps'])
actions=[]
for name in ['ActionChangeWarpsToNegativeStops','ActionChangeNegativeStopsToWarps']:
 text=source('EditorActions/'+name+'.cs');ctors=[]
 for m in re.finditer(r'public '+name+r'\(',text):
  ctor=member(text[m.start():],'public '+name+'(').replace(': base(false, false)','');ctors.append(ctor)
 actions.append('class '+name+' {private List<EditorEvent> OriginalEvents;private List<EditorEvent> NewEvents;private Editor Editor;private EditorChart Chart;public int[] Rows()=>OriginalEvents.Select(e=>e.Row).ToArray();'+''.join(ctors)+'}')
adapter='''using System.Text.Json;namespace StepManiaEditor {
class EditorEvent {public int Row;public bool Selected;public EditorEvent(int row){Row=row;}public bool IsSelected()=>Selected;public void Select()=>Selected=true;public void Deselect()=>Selected=false;public bool IsAddedToChart()=>true;}
class EditorPatternEvent(int row):EditorEvent(row){}class EditorWarpEvent(int row):EditorEvent(row){}class EditorStopEvent(int row,double seconds):EditorEvent(row){public double GetStopLengthSeconds()=>seconds;}
class EditorChart {public bool Editable=true;public bool CanBeEdited()=>Editable;public List<EditorEvent> Events=[];public IEnumerable<EditorWarpEvent> GetWarps()=>Events.OfType<EditorWarpEvent>();public IEnumerable<EditorStopEvent> GetStops()=>Events.OfType<EditorStopEvent>();}
class ActiveEditorChart {public EditorChart Chart=new();public Selection Selection=new();public EditorChart GetChart()=>Chart;public Selection GetSelection()=>Selection;}
class Editor {public ActiveEditorChart Focused=new();public ActiveEditorChart GetFocusedChartData()=>Focused;public EditorChart GetFocusedChart()=>Focused.Chart;}
class ActionQueue {public static ActionQueue Instance=new();public List<object> Calls=[];public void Do(object action)=>Calls.Add(action);}
class UIEditEvents {Editor Editor;public UIEditEvents(Editor e){Editor=e;} METHODS }
ACTIONS
class Program {static void Main(){var output=new List<object>();foreach(var mode in new[]{"mixed","empty","readonly","missing","reused","cleared"}){var e=new Editor();var s=e.Focused.Selection;var w96=new EditorWarpEvent(96);var w48=new EditorWarpEvent(48);var w144=new EditorWarpEvent(144);var w192=new EditorWarpEvent(192);var w240=new EditorWarpEvent(240);var n96=new EditorStopEvent(96,-1);var n48=new EditorStopEvent(48,-2);var p=new EditorStopEvent(72,1);var tap=new EditorEvent(24);e.Focused.Chart.Events=[w48,w96,w144,w192,w240,n96,n48,p,tap];if(mode!="empty"){foreach(var ev in new EditorEvent[]{w96,tap,w48,n96,p,n48})s.SelectEvent(ev,true);}if(mode=="reused"){s.ClearSelectedEvents();foreach(var ev in new[]{w48,w96,w144})s.SelectEvent(ev,true);s.DeselectEvent(w96);s.DeselectEvent(w48);s.SelectEvent(w192,true);s.SelectEvent(w240,true);}if(mode=="cleared"){s.ClearSelectedEvents();s.SelectEvent(w96,true);s.SelectEvent(w48,true);s.SelectEvent(w96,true);}var order=s.GetSelectedEvents().Select(ev=>new{Kind=ev.GetType().Name,ev.Row}).ToArray();if(mode=="readonly")e.Focused.Chart.Editable=false;if(mode=="missing")e.Focused=null;ActionQueue.Instance.Calls.Clear();var ui=new UIEditEvents(e);ui.ConvertSelectedWarpsToNegativeStops();ui.ConvertSelectedNegativeStopsToWarps();output.Add(new{Mode=mode,Order=order,Calls=ActionQueue.Instance.Calls.Select(v=>new{Kind=v.GetType().Name,Rows=v is ActionChangeWarpsToNegativeStops w?w.Rows():((ActionChangeNegativeStopsToWarps)v).Rows()}).ToArray()});}Console.Write(JsonSerializer.Serialize(output));}}
}'''.replace('METHODS',methods).replace('ACTIONS','\n'.join(actions))
with tempfile.TemporaryDirectory(prefix='selected-timing-oracle-') as d:
 t=Path(d);(t/'Selection.cs').write_text(selection);(t/'Program.cs').write_text(adapter);(t/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>');subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'SourcesSha256':{n:hashlib.sha256(v.encode()).hexdigest() for n,v in sources.items()},'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':sha,'Scope':'Whole source Selection; original TryGetFocusedChartSelection, both selected callbacks, both conversion routing helpers and all four original constructors. Recording action queue only. Arithmetic/Undo engine covered by pre-existing original full-action timing-conversion oracle; not executed here.'},indent=2)+'\n');print('Executed',len(result),'original selected callback/selection cases.')
