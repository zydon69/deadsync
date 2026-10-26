#!/usr/bin/env python3
"""Execute original lane-down/movement/cancel control flow with adapted event/action storage."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def extract(s,m):
 start=s.index(m);pos=s.index('{',start);end=pos+1;depth=1
 while depth:depth+=(s[end]=='{')-(s[end]=='}');end+=1
 return s[start:end]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=a.reference_root.resolve()
 paths={'active':r/'StepManiaEditor/ActiveEditorChart.cs','position':r/'StepManiaEditor/EditorPosition.cs','editor':r/'StepManiaEditor/Editor.cs'}
 for path in paths.values():assert path.read_bytes()==subprocess.check_output(['git','-C',str(r),'show',f'{PIN}:{path.relative_to(r)}'])
 s={k:p.read_text(encoding='utf-8-sig') for k,p in paths.items()}
 m={k:extract(s[f],marker) for k,f,marker in [('down','active','public void OnLaneInputDown('),('set','active','private void SetLaneInputDownNote('),('move','active','public void UpdateLaneEditStatesFromPosition('),('cancel','active','public bool CancelLaneInput('),('nearest','position','public int GetNearestRow()\n\t{'),('advance','editor','public void MoveLaterInChart()')]}
 def op(kind,**kw):return dict(kind=kind,**kw)
 cases=[dict(name='tap-hold-backward-return-and-cancel',initial=None,steps=[op('down',row=48,player=0,snap=0,playing=False,advance=False),op('move',row=96,player=1),op('move',row=0,player=1),op('move',row=48,player=1),op('cancel')]),dict(name='initial-delete-to-hold-and-back',initial=48,steps=[op('down',row=48,player=0,snap=0,playing=False,advance=False),op('move',row=96,player=1),op('move',row=48,player=1),op('up')]),dict(name='nearest-half-then-snap-half',initial=None,steps=[op('down',row=12.5,player=0,snap=24,playing=False,advance=False),op('move',row=25.5,player=0),op('up')]),dict(name='playing-commits-immediately',initial=None,steps=[op('down',row=24,player=0,snap=48,playing=True,advance=True),op('move',row=96,player=1),op('up')]),dict(name='advance-by-snap',initial=None,steps=[op('down',row=12.5,player=0,snap=24,playing=False,advance=True)]),dict(name='initial-deletion-does-not-advance',initial=48,steps=[op('down',row=48,player=0,snap=48,playing=False,advance=True),op('up')]),dict(name='negative-leadin-does-not-create',initial=None,steps=[op('down',row=-0.5,player=0,snap=48,playing=False,advance=False),op('up')])]
 program='''using System;using System.Collections.Generic;using System.Text.Json;
enum NoteEntryMode{Normal,AdvanceBySnap}
class Preferences{public static Preferences Instance=new();public NoteEntryMode NoteEntryMode;public int SnapIndex,Snap;public Scroll PreferencesScroll=new();public class Scroll{public bool StopPlaybackWhenScrolling=false;}public object PreferencesKeyBinds=new Keys();public class Keys{public object ArrowModification;}}
class KeyCommandManager{public static bool Modified;public static bool IsAnyInputDown(object key)=>Modified;}
class EditorEvent{public int Row,Lane,Player;public int GetRow()=>Row;public int GetPlayer()=>Player;public static EditorEvent CreateEvent(EventConfig c)=>c.Mine?new EditorMineNoteEvent{Row=c.Row,Lane=c.Lane,Player=c.Player}:new EditorTapNoteEvent{Row=c.Row,Lane=c.Lane,Player=c.Player};}
class EditorTapNoteEvent:EditorEvent{}class EditorMineNoteEvent:EditorEvent{}class EditorHoldNoteEvent:EditorEvent{public int End;public int GetEndRow()=>End;}
class EventConfig{public int Row,Lane,Player;public bool Mine,IsBeingEdited;public static EventConfig CreateTapConfig(EditorChart c,int r,int l,int p)=>new(){Row=r,Lane=l,Player=p};public static EventConfig CreateMineConfig(EditorChart c,int r,int l,int p)=>new(){Row=r,Lane=l,Player=p,Mine=true};}
class ActionDeleteEditorEvents{public EditorEvent Event;public ActionDeleteEditorEvents(EditorEvent e){Event=e;}}
class LaneEditState{bool Active;int Start;EditorEvent Event;bool Delete;
public bool IsActive()=>Active;public int GetStartingRow()=>Start;public EditorEvent GetEventBeingEdited()=>Event;public bool IsOnlyDelete()=>Active&&Delete&&Event==null;
public void Clear(bool done){Event=null;if(done){Active=false;Delete=false;Start=0;}}
public void StartEditingWithDelete(int row,ActionDeleteEditorEvents action){Active=true;Start=row;Delete=true;Event=null;}
public void SetEditingTapOrMine(EditorEvent note){Active=true;Start=note.Row;Event=note;}
public void SetEditingHold(EditorChart c,int row,int lane,int player,int start,int length,bool roll){Active=true;Start=start;Event=new EditorHoldNoteEvent{Row=row,Lane=lane,Player=player,End=row+length};}
}
class Events{public EditorEvent Initial;public EditorEvent FindNoteAt(int row,int lane,bool ignore)=>Initial?.Row==row?Initial:null;}
class EditorChart{public int NumInputs=1;public Events Events=new();public Events GetEvents()=>Events;}
class Position{public double ChartPosition;
'''+m['nearest']+'''}
class Receptor{public void OnInputDown(){}public void OnInputUp(){}}
class SnapManager{public static int GetCurrentRows()=>Preferences.Instance.Snap;}
class Host{public int Player;public Position Position;public int GetPlayer(EditorChart c)=>Player;Position GetPosition()=>Position;void SetChartPosition(int row){Position.ChartPosition=row;}void StopPlayback(){}void OnMoveToNextMeasure(){throw new Exception("No measure fallback in these cases");}void UpdateAutoPlayFromScrolling(){}
'''+m['advance']+'''}
class Active{public EditorChart Chart=new();public Position Position=new();public Host Editor=new();public LaneEditState[] LaneEditStates={new()};public Receptor[] Receptors={new()};public List<object> Commits=new();
public void OnLaneInputUp(int lane){if(!LaneEditStates[lane].IsActive())return;Commits.Add(SnapshotDraft());LaneEditStates[lane].Clear(true);}
object SnapshotDraft(){var e=LaneEditStates[0].GetEventBeingEdited();return e==null?null:new{row=e.Row,player=e.Player,end=e is EditorHoldNoteEvent h?(int?)h.End:null,kind=e is EditorHoldNoteEvent?"Hold":e is EditorMineNoteEvent?"Mine":"Tap"};}
public object Snapshot()=>new{active=LaneEditStates[0].IsActive(),draft=SnapshotDraft(),commits=Commits.Count,position=Position.ChartPosition};
'''+m['down']+m['set']+m['move']+m['cancel']+'''
static void Main(){var requests=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var result=new List<object>();foreach(var request in requests.EnumerateArray()){var c=new Active();c.Editor.Position=c.Position;if(request.GetProperty("initial").ValueKind!=JsonValueKind.Null)c.Chart.Events.Initial=new EditorTapNoteEvent{Row=request.GetProperty("initial").GetInt32()};var steps=new List<object>();foreach(var step in request.GetProperty("steps").EnumerateArray()){var kind=step.GetProperty("kind").GetString();if(step.TryGetProperty("row",out var row))c.Position.ChartPosition=row.GetDouble();if(step.TryGetProperty("player",out var player))c.Editor.Player=player.GetInt32();switch(kind){case "down":var p=Preferences.Instance;p.Snap=step.GetProperty("snap").GetInt32();p.SnapIndex=p.Snap==0?0:1;p.NoteEntryMode=step.GetProperty("advance").GetBoolean()?NoteEntryMode.AdvanceBySnap:NoteEntryMode.Normal;c.OnLaneInputDown(0,step.GetProperty("playing").GetBoolean(),p.Snap);break;case "move":c.UpdateLaneEditStatesFromPosition();break;case "cancel":c.CancelLaneInput();break;case "up":c.OnLaneInputUp(0);break;}steps.Add(c.Snapshot());}result.Add(steps);}Console.Write(JsonSerializer.Serialize(result));}}
'''
 # Preserve source member shape while exposing only the ArrowModification stub.
 program=program.replace('public object PreferencesKeyBinds=new Keys();','public Keys PreferencesKeyBinds=new Keys();')
 with tempfile.TemporaryDirectory(prefix='deadsync-lane-entry-') as folder:
  d=Path(folder);(d/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(d/'Program.cs').write_text(program)
  b=subprocess.run([str(a.dotnet),'build',str(d/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
  if b.returncode:
   Path('/tmp/deadsync-lane-entry-Program.cs').write_text(program);raise SystemExit(b.stdout+b.stderr)
  o=subprocess.run([str(a.dotnet),str(d/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),capture_output=True,text=True,check=True)
 doc=dict(reference_commit=PIN,source_bytes_verified_against_git=True,source_sha256={str(p.relative_to(r)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths.values()},method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in m.items()},scope='Original OnLaneInputDown, SetLaneInputDownNote, UpdateLaneEditStatesFromPosition, CancelLaneInput, GetNearestRow and MoveLaterInChart. Adapted LaneEditState event/action storage and release recorder: does not execute original undo actions, overlap resolution, timing or full chart tree.',cases=[dict(**c,expected=e) for c,e in zip(cases,json.loads(o.stdout))])
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(len(cases),'original lane input control-flow sequences')
if __name__=='__main__':main()
