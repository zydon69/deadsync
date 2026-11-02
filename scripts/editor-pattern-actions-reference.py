#!/usr/bin/env python3
"""Execute source pattern dispatch + actual Fumen reseed/history actions.
Generation work is an explicit recording adapter, not a second generator.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
GA='48e18f71655d0f0d86144c63239331833501fda2';FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89';LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
for directory,pin in [(a.editor_root,GA),(a.reference_root,LIB),(a.reference_root/'Fumen',FUMEN)]:assert subprocess.check_output(['git','-C',str(directory),'rev-parse','HEAD'],text=True).strip()==pin
sources={}
def source(path):
 raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/{path}']);sources[path]=hashlib.sha256(raw).hexdigest();return raw.decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);end=text.index('{',start)+1;depth=1
 while depth:
  depth+=(text[end]=='{')-(text[end]=='}');end+=1
 return text[start:end]
editor=source('Editor.cs');selection=source('Selection.cs');names=['OnRegenerateSelectedPatterns()','OnRegenerateSelectedPatternsWithNewSeeds()','OnRegenerateAllPatterns()','OnRegenerateAllPatternsWithNewSeeds()','RegeneratePatterns(IEnumerable<EditorPatternEvent> patterns, bool useNewSeeds)']
methods='\n'.join(member(editor,'private void '+name).replace('private void','public void',1) for name in names)
code='''using Fumen;namespace StepManiaEditor;
class EditorEvent {bool selected;public bool IsSelected()=>selected;public void Select()=>selected=true;public void Deselect()=>selected=false;public bool IsAddedToChart()=>true;}
class EditorPatternEvent(int row,int seed):EditorEvent {public int Row=row;public int RandomSeed{get;set;}=seed;}
class EditorChart {public List<EditorPatternEvent> Patterns=new(){new(0,10),new(192,20),new(384,30)};public IEnumerable<EditorPatternEvent> GetPatterns()=>Patterns;}
class Active(Selection selection){public Selection GetSelection()=>selection;}
class Editor {public EditorChart FocusedChart=new();public Selection Selection=new();public Active GetFocusedChartData()=>new(Selection);bool EditEarlyOut()=>false; METHODS }
// Only asynchronous worker scheduling is adapted. The actual source dispatch,
// Selection, ActionMultiple, ActionSet* and ActionQueue execute unchanged.
class ActionAutoGeneratePatterns(Editor editor,EditorChart chart,IEnumerable<EditorPatternEvent> patterns):UndoableAction(false,false){public static List<object> Scheduled=new();public override bool AffectsFile()=>true;protected override void DoImplementation(){Scheduled.Add(patterns.OrderBy(p=>p.Row).Select(p=>new {p.Row,p.RandomSeed}).ToArray());}protected override void UndoImplementation(){} }
'''.replace('METHODS',methods)
program='''using Fumen;using StepManiaEditor;using System.Text.Json;
class Program {static object Snapshot(Editor e)=>e.FocusedChart.Patterns.Select(p=>new{p.Row,p.RandomSeed}).ToArray();static void Main(){var result=new List<object>();foreach(var action in new[]{"SelectedFixed","SelectedNew","AllFixed","AllNew","EmptyFixed","EmptyNew"}){ActionQueue.Instance.Clear();ActionAutoGeneratePatterns.Scheduled.Clear();var e=new Editor();if(!action.StartsWith("Empty")){e.Selection.SelectEvent(e.FocusedChart.Patterns[0],true);e.Selection.SelectEvent(new EditorEvent(),true);e.Selection.SelectEvent(e.FocusedChart.Patterns[2],true);}var before=Snapshot(e);switch(action){case "SelectedFixed":case "EmptyFixed":e.OnRegenerateSelectedPatterns();break;case "SelectedNew":case "EmptyNew":e.OnRegenerateSelectedPatternsWithNewSeeds();break;case "AllFixed":e.OnRegenerateAllPatterns();break;case "AllNew":e.OnRegenerateAllPatternsWithNewSeeds();break;}var after=Snapshot(e);var scheduled=ActionAutoGeneratePatterns.Scheduled.ToArray();var undo1=ActionQueue.Instance.Undo()?.GetType().Name;var afterUndo1=Snapshot(e);var undo2=ActionQueue.Instance.Undo()?.GetType().Name;var afterUndo2=Snapshot(e);ActionQueue.Instance.Redo();ActionQueue.Instance.Redo();result.Add(new{Action=action,Before=before,After=after,Scheduled=scheduled,Undo1=undo1,AfterUndo1=afterUndo1,Undo2=undo2,AfterUndo2=afterUndo2,AfterRedo=Snapshot(e)});}Console.Write(JsonSerializer.Serialize(result));}}
'''
with tempfile.TemporaryDirectory(prefix='deadsync-pattern-action-source-') as folder:
 root=Path(folder);(root/'Adapter.cs').write_text(code);(root/'Selection.cs').write_text(selection);(root/'Program.cs').write_text(program)
 (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{a.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);result=json.loads(subprocess.check_output([str(a.dotnet),str(root/'bin/Release/net10.0/Oracle.dll')],text=True));assembly_hash=hashlib.sha256((root/'bin/Release/net10.0/Oracle.dll').read_bytes()).hexdigest()
for case in result:
 if case['Action'].startswith('Empty'):assert not case['Scheduled'] and case['Undo1'] is None
 elif case['Action'].endswith('New'):
  assert case['Undo1']=='ActionAutoGeneratePatterns' and case['Undo2']=='ActionMultiple';assert case['AfterUndo1']==case['After'];assert case['AfterUndo2']==case['Before'];assert case['AfterRedo']==case['After']
 else:assert case['Undo1']=='ActionAutoGeneratePatterns' and case['Undo2'] is None
 if case['Action'].startswith('Selected'):assert [p['Row'] for p in case['Scheduled'][0]]==[0,384];assert case['After'][1]==case['Before'][1]
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':GA,'LibraryCommit':LIB,'FumenCommit':FUMEN,'SourcesSha256':sources,'AdapterSha256':hashlib.sha256((code+program).encode()).hexdigest(),'OracleAssemblySha256':assembly_hash,'FumenAssemblySha256':hashlib.sha256((a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()).hexdigest(),'Scope':'Original four callbacks, RegeneratePatterns and complete Selection; actual pinned Fumen ActionQueue/ActionMultiple/ActionSetObjectFieldOrPropertyValue. Recording generation adapter proves scheduling/history only; generation and clear workers have separate existing source oracles.'},indent=2)+'\n');print(f'Executed {len(result)} source callback/history cases.')
