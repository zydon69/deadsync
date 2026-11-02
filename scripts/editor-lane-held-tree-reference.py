#!/usr/bin/env python3
"""Execute pinned LaneEditState + real Add actions against explicit identity-list storage."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=a.reference_root.resolve()
 files=['StepManiaEditor/LaneEditState.cs','StepManiaEditor/EditorActions/ActionAddEditorEvent.cs','StepManiaEditor/EditorActions/ActionAddHoldEvent.cs']
 sources={f:(r/f).read_text(encoding='utf-8-sig') for f in files}
 for f in files:assert (r/f).read_bytes()==subprocess.check_output(['git','-C',str(r),'show',f'{PIN}:{f}'])
 host=r'''using System;using System.Linq;using System.Collections.Generic;using System.Text.Json;
namespace Fumen { }
namespace StepManiaEditor {
abstract class UndoableAction{protected UndoableAction(bool x,bool y){}public void Do()=>DoImplementation();public void Undo()=>UndoImplementation();protected abstract void DoImplementation();protected abstract void UndoImplementation();public abstract bool AffectsFile();}
class ActionMultiple:UndoableAction{List<UndoableAction>A=new();public ActionMultiple():base(false,false){}public List<UndoableAction>GetActions()=>A;public void EnqueueAndDo(UndoableAction a){A.Add(a);a.Do();}protected override void DoImplementation(){foreach(var a in A)a.Do();}protected override void UndoImplementation(){foreach(var a in A.AsEnumerable().Reverse())a.Undo();}public override bool AffectsFile()=>true;}
class ActionQueue{public static ActionQueue Instance=new();public int Count;public void Do(UndoableAction a){a.Do();Count++;}}
class Delete:UndoableAction{EditorEvent E;public Delete(EditorEvent e):base(false,false){E=e;}protected override void DoImplementation()=>E.C.DeleteEvent(E);protected override void UndoImplementation()=>E.C.AddEvent(E);public override bool AffectsFile()=>true;}
class EditorChart{public List<EditorEvent> Events=new();public void AddEvent(EditorEvent e){if(Events.Contains(e))throw new Exception("duplicate reference");Events.Add(e);}public void DeleteEvent(EditorEvent e){if(!Events.Remove(e))throw new Exception("missing reference");}}
class EventConfig{public EditorChart C;public int Row,Lane,Player,Length;public string Kind;public bool IsBeingEdited;public static EventConfig CreateHoldConfig(EditorChart c,int r,int l,int p,int n,bool roll)=>new(){C=c,Row=r,Lane=l,Player=p,Length=n,Kind=roll?"Roll":"Hold"};public static EventConfig CreateTapConfig(EditorEvent e)=>new(){C=e.C,Row=e.Row,Lane=e.Lane,Player=e.Player,Kind="Tap"};public static EventConfig CreateMineConfig(EditorEvent e)=>new(){C=e.C,Row=e.Row,Lane=e.Lane,Player=e.Player,Kind="Mine"};}
class EditorEvent{public EditorChart C;public int Row,Lane,Player;public bool Edited;public string Kind;public int GetRow()=>Row;public int GetLane()=>Lane;public int GetPlayer()=>Player;public EditorChart GetEditorChart()=>C;public string GetShortTypeName()=>Kind;public bool IsLaneNote()=>true;public void SetIsBeingEdited(bool x){Edited=x;}public static EditorEvent CreateEvent(EventConfig c){EditorEvent e=c.Kind=="Hold"||c.Kind=="Roll"?new EditorHoldNoteEvent{Length=c.Length}:c.Kind=="Mine"?new EditorMineNoteEvent():new EditorTapNoteEvent();e.C=c.C;e.Row=c.Row;e.Lane=c.Lane;e.Player=c.Player;e.Kind=c.Kind;e.Edited=c.IsBeingEdited;return e;}}
class EditorTapNoteEvent:EditorEvent{}class EditorMineNoteEvent:EditorEvent{}class EditorHoldNoteEvent:EditorEvent{public int Length;public void SetIsRoll(bool b){Kind=b?"Roll":"Hold";}}
class Program{
static object Snap(EditorChart c,LaneEditState s)=>new{active=s.IsActive(),onlyDelete=s.IsOnlyDelete(),events=c.Events.OrderBy(e=>e.Row).ThenBy(e=>e.Lane).ThenBy(e=>e.Player).ThenBy(e=>e.Edited).Select(e=>new{row=e.Row,lane=e.Lane,player=e.Player,kind=e.Kind,end=e is EditorHoldNoteEvent h?(int?)(e.Row+h.Length):null,edited=e.Edited}).ToArray()};
static void Main(){var cases=new List<object>();
foreach(var variant in new[]{"duplicate-head","overlap-held","initial-delete-return","modifier","cancel","commit"}){
var c=new EditorChart();var original=EditorEvent.CreateEvent(new(){C=c,Row=48,Lane=0,Player=0,Kind="Tap"});c.AddEvent(original);var s=new LaneEditState();var states=new List<object>{Snap(c,s)};
if(variant=="initial-delete-return")s.StartEditingWithDelete(48,new Delete(original));else s.SetEditingTapOrMine(EditorEvent.CreateEvent(new(){C=c,Row=48,Lane=0,Player=0,Kind="Tap",IsBeingEdited=true}));states.Add(Snap(c,s));
if(variant!="duplicate-head"){s.SetEditingHold(c,24,0,0,48,96,false);states.Add(Snap(c,s));}
if(variant=="initial-delete-return"){s.Clear(false);states.Add(Snap(c,s));}
if(variant=="modifier"){s.ArrowModificationKeyPressed(false);states.Add(Snap(c,s));s.ArrowModificationKeyPressed(true);states.Add(Snap(c,s));}
if(variant=="cancel"){s.Clear(true);states.Add(Snap(c,s));}
if(variant=="commit"){s.Commit();states.Add(Snap(c,s));}
cases.Add(new{name=variant,states});}
Console.Write(JsonSerializer.Serialize(cases));}}
}
'''
 with tempfile.TemporaryDirectory(prefix='deadsync-lane-held-tree-') as tmp:
  d=Path(tmp);(d/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(d/'Host.cs').write_text(host)
  for i,s in enumerate(sources.values()):(d/f'Source{i}.cs').write_text(s)
  result=subprocess.run([str(a.dotnet),'build',str(d/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if result.returncode:raise SystemExit(result.stdout+result.stderr)
  output=subprocess.check_output([str(a.dotnet),str(d/'bin/Debug/net10.0/oracle.dll')],text=True)
 doc={'reference_commit':PIN,'source_bytes_verified':True,'source_sha256':{f:hashlib.sha256((r/f).read_bytes()).hexdigest() for f in files},'scope':'Entire original LaneEditState, ActionAddEditorEvent and ActionAddHoldEvent. Explicit identity-list chart/event factory and undo-composition/queue host. Does not execute ActiveEditorChart.OnLaneInputUp overlap resolver, full tree comparator or Fumen reader. Establishes pending action timing, duplicate identities and cancellation only.','cases':json.loads(output)}
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(len(doc['cases']),'real LaneEditState/action sequences')
if __name__=='__main__':main()
