#!/usr/bin/env python3
"""Real source hold discovery + unchanged forward admission, on real Fumen tree."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2'
def source(n):return subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/'+n]).decode('utf-8-sig')
def member(s,m):
 i=s.index(m);j=s.index('{',i)+1;d=1
 while d:d+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[i:j]
active=source('ActiveEditorChart.cs');chart=source('EditorChart.cs');start=active.index('while (enumerator.MoveNext())',active.index('// Now we can scan forward'));loop=member(active[start:],'while (enumerator.MoveNext())')
src=r'''using System;using System.Collections.Generic;using System.Linq;using System.Text.Json;using Fumen;
interface IChartRegion{}
class EditorEvent:IComparable<EditorEvent>{public int Id,Row,Lane;public float Alpha;public int GetRow()=>Row;public int GetLane()=>Lane;public virtual int GetRowDuration()=>0;public bool IsBeingEdited()=>false;public bool IsLaneNote()=>true;public bool IsMiscEvent()=>false;public void SetDimensions(double x,double y,double w,double h,double z){}public int CompareTo(EditorEvent e)=>Row.CompareTo(e.Row);}
class EditorHoldNoteEvent:EditorEvent{public int End;public override int GetRowDuration()=>End-Row;public void SetHoldStartHeight(double h){}}
class Tree:RedBlackTree<EditorEvent>{public IReadOnlyRedBlackTree<EditorEvent>.IReadOnlyRedBlackTreeEnumerator FindGreatestAtOrBeforeChartPosition(double row)=>FindGreatestPreceding(new EditorEvent{Row=(int)row},true);}
class EditorChart{public int NumInputs=4;public Tree EditorEvents=new();
'''+member(chart,'public EditorHoldNoteEvent[] GetHoldsOverlappingPosition(')+r'''}
class EditState{public bool IsActive()=>false;public EditorEvent GetEventBeingEdited()=>null;}
class Spacing{public double GetY(EditorEvent e,double y)=>(e.Row-166)*2+200;public bool IsScrollRateNegative()=>false;public void UpdatePpsAndPpr(EditorEvent e,double i,double z){}}
class Layout{public void PositionEvent(EditorEvent e,double y){}}
class Host{public EditorChart Chart=new();public EditState[] LaneEditStates=Array.Empty<EditState>();public Spacing SpacingHelper=new();public Layout MiscEventWidgetLayoutManager=new();
'''+member(active,'public List<EditorHoldNoteEvent> ScanBackwardsForHolds(')+r'''
bool ShouldDrawMiscEvents()=>true;bool IsBeyondVisibleFarEdge(double y,double h,double d)=>y>h;void AddRegion(IChartRegion r,bool b,ref List<IChartRegion>x,ref HashSet<IChartRegion>y,double p,EditorEvent n,double a,double w){}void UpdateRegions(ref List<IChartRegion>x,ref HashSet<IChartRegion>y,double p,EditorEvent n,EditorEvent v){}void UpdateBeatMarkers(EditorEvent r,ref int x,ref int y,EditorEvent n,double a,double z,double p,double h,int m){}void CheckForCompletingHolds(HashSet<EditorHoldNoteEvent> h,double p,EditorEvent n){}bool CheckForCompletingHold(EditorHoldNoteEvent h,double p,EditorEvent n)=>true;
public object Run(int max){Chart=new();var hold=new EditorHoldNoteEvent{Id=0,Row=0,Lane=0,End=192};Chart.EditorEvents.Insert(hold);Chart.EditorEvents.Insert(new EditorEvent{Id=1,Row=96,Lane=1});var enumerator=Chart.EditorEvents.FindGreatestPreceding(new EditorEvent{Row=50});var pre=ScanBackwardsForHolds(enumerator,50.0);var noteEvents=pre.Cast<EditorEvent>().ToList();var same=pre.Count==1&&Object.ReferenceEquals(pre[0],hold);var rateEnumerator=new List<EditorEvent>().GetEnumerator();bool hasNextRateEvent=false;EditorEvent nextRateEvent=null,rateEvent=null;var holdsNeedingToBeCompleted=new HashSet<EditorHoldNoteEvent>();var regionsNeedingToBeAdded=new List<IChartRegion>();var addedRegions=new HashSet<IChartRegion>();double previousRateEventY=0,interpolatedScrollRate=1,spacingZoom=1,startPosX=0,arrowW=64,arrowH=64,sizeZoom=1,screenHeight=200,directionSign=1;float miscEventAlpha=1,noteAlpha=1;int beatMarkerRow=0,beatMarkerLastRecordedRow=-1,numArrows=4,maxMarkersToDraw=256,numRateAlteringEventsProcessed=1,maxEventsToDraw=max;bool reachedEndOfScreen=false;
'''+loop+r'''
return new{max,pre=pre.Select(e=>e.Id),same,admitted=noteEvents.Select(e=>e.Id),count=noteEvents.Count};}}
class Program{static void Main(){var h=new Host();Console.WriteLine(JsonSerializer.Serialize(new[]{-1,0,1,2,5}.Select(h.Run)));}}
'''
dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
assert subprocess.check_output(['git','-C',str(a.reference_root/'Fumen'),'rev-parse','HEAD'],text=True).strip()=='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
with tempfile.TemporaryDirectory(prefix='editor-hold-source-')as d:
 d=Path(d);(d/'Program.cs').write_text(src);(d/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>false</EnableDefaultCompileItems></PropertyGroup><ItemGroup><Compile Include="Program.cs"/><Reference Include="Fumen"><HintPath>'+str(dll)+'</HintPath></Reference></ItemGroup></Project>');q=subprocess.run([str(a.dotnet),'run','--project',str(d/'Oracle.csproj'),'-c','Release','--verbosity','quiet'],capture_output=True,text=True)
 if q.returncode:raise RuntimeError(q.stdout+q.stderr)
 cases=json.loads(q.stdout[q.stdout.index('[{"'):]);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(cases,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':'0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89','SourceMembers':['EditorChart.GetHoldsOverlappingPosition','ActiveEditorChart.ScanBackwardsForHolds','UpdateChartEvents unchanged forward while'],'SourceSha256':{'EditorChart.cs':hashlib.sha256(chart.encode()).hexdigest(),'ActiveEditorChart.cs':hashlib.sha256(active.encode()).hexdigest()},'FumenDLLSha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'Host':'real Fumen tree; strict predecessor query atrow50, explicit scalar hold0..192/lane0 andtap96/lane1; empty edit states; geometry callbacks as scalar host; not whole source reader'},indent=2)+'\n');print(json.dumps(cases))
