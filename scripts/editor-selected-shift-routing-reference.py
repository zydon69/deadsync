#!/usr/bin/env python3
"""Execute original selected-shift callbacks; arithmetic has a separate full-action oracle."""
import argparse, hashlib, json, re, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser()
for name in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fumen='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for directory,expected in [(a.editor_root,pin),(a.reference_root/'Fumen',fumen)]:assert subprocess.check_output(['git','-C',str(directory),'rev-parse','HEAD'],text=True).strip()==expected
sources={}
def source(name):
 raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/'+name]);sources[name]=hashlib.sha256(raw).hexdigest();return raw.decode('utf-8-sig')
def member(text,sig):
 start=text.index(sig);i=text.index('{',start);j=i+1;depth=1
 while depth:
  if text[j]=='{':depth+=1
  if text[j]=='}':depth-=1
  j+=1
 return text[start:j]
editor=source('Editor.cs');active=source('ActiveEditorChart.cs');selection=source('Selection.cs')
methods='\n'.join(member(editor,sig) for sig in ['public int GetShiftNotesRows()','public void OnShiftSelectedNotesEarlier()','public void OnShiftSelectedNotesLater()'])
active_methods='\n'.join(member(active,sig) for sig in ['public void OnShiftSelectedNotesEarlier(int rows)','public void OnShiftNotesEarlier(IEnumerable<EditorEvent> events, int rows)','public void OnShiftSelectedNotesLater(int rows)','public void OnShiftNotesLater(IEnumerable<EditorEvent> events, int rows)'])
raw=subprocess.check_output(['git','-C',str(a.reference_root/'Fumen'),'show',fumen+':Fumen/Converters/SMCommon.cs']);sources['Fumen/SMCommon.cs']=hashlib.sha256(raw).hexdigest();constant=re.search(r'public const int MaxValidDenominator\s*=\s*(\d+)\s*;',raw.decode()).group(1)
code='''using System.Text.Json; namespace StepManiaEditor {
class EditorEvent {public int Row;public bool Selected;public EditorEvent(int row){Row=row;}public bool IsSelected()=>Selected;public void Select()=>Selected=true;public void Deselect()=>Selected=false;public bool IsAddedToChart()=>true;}
class EditorPatternEvent(int row):EditorEvent(row){}
class Snap {public int Rows;public int GetCurrentRows()=>Rows;}
class Editor {public const int MaxValidDenominator=CONSTANT;public Snap SnapManager=new();public bool Blocked;public ActiveEditorChart Focused;bool EditEarlyOut()=>Blocked;public ActiveEditorChart GetFocusedChartData()=>Focused; METHODS }
class ActiveEditorChart {public Editor Editor;public object Chart=new();public Selection Selection=new(); ACTIVE }
class ActionShiftSelectionRow {public int Rows;public int[] Selected;public ActionShiftSelectionRow(Editor e,object chart,IEnumerable<EditorEvent> events,int rows){Rows=rows;Selected=events.Select(e=>e.Row).ToArray();}}
class ActionQueue {public static ActionQueue Instance=new();public List<ActionShiftSelectionRow> Calls=[];public void Do(ActionShiftSelectionRow action)=>Calls.Add(action);}
class Program {static void Main(){var output=new List<object>();foreach(var snap in new[]{48,24,16,12,8,6,4,3,1,0})foreach(var later in new[]{false,true})foreach(var mode in new[]{"selected","empty","readonly","unfocused"}){var e=new Editor();e.SnapManager.Rows=snap;var c=new ActiveEditorChart{Editor=e};e.Focused=c;if(mode!="empty"){c.Selection.SelectEvent(new EditorEvent(96),true);c.Selection.SelectEvent(new EditorEvent(24),true);}if(mode=="readonly")e.Blocked=true;if(mode=="unfocused")e.Focused=null;ActionQueue.Instance.Calls.Clear();if(later)e.OnShiftSelectedNotesLater();else e.OnShiftSelectedNotesEarlier();output.Add(new{Snap=snap,Later=later,Mode=mode,Calls=ActionQueue.Instance.Calls.Select(v=>new{v.Rows,v.Selected}).ToArray()});}Console.Write(JsonSerializer.Serialize(output));}}
}'''.replace('CONSTANT',constant).replace('METHODS',methods).replace('ACTIVE',active_methods)
with tempfile.TemporaryDirectory(prefix='selected-shift-routing-') as folder:
 root=Path(folder);(root/'Selection.cs').write_text(selection);(root/'Program.cs').write_text(code);(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>');subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=root/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(EditorCommit=pin,FumenCommit=fumen,MaxValidDenominator=int(constant),SourcesSha256=sources,AdapterSha256=hashlib.sha256(code.encode()).hexdigest(),AssemblySha256=sha,Scope='Original whole Selection, Editor.GetShiftNotesRows and both selected callbacks, four ActiveEditorChart forwarding methods. Adapter supplies edit guard, snap rows, focused chart and recording action constructor. No movement arithmetic, UI, history or chart parser executed; separate event-shift original-action oracle covers arithmetic.'),indent=2)+'\n');print('Executed',len(result),'original selected shift routing cases.')
