#!/usr/bin/env python3
"""Run the unchanged UpdateChartEvents admission loop with typed geometry hosts.
The original whole while loop is compiled, not a transcription of its quota.
It covers admission after source start-row/hold discovery, not those searches.
"""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
pin='48e18f71655d0f0d86144c63239331833501fda2'
s=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/ActiveEditorChart.cs']).decode('utf-8-sig')
start=s.index('while (enumerator.MoveNext())',s.index('// Now we can scan forward'));end=s.index('{',start)+1;depth=1
while depth:depth+=(s[end]=='{')-(s[end]=='}');end+=1
body=s[start:end]
host=r'''using System;using System.Collections.Generic;using System.Linq;using System.Text.Json;
interface IChartRegion{}
class EditorEvent{public int Id;public double Y;public bool Lane,Misc;public float Alpha;public bool IsLaneNote()=>Lane;public bool IsMiscEvent()=>Misc;public int GetLane()=>0;public void SetDimensions(double x,double y,double w,double h,double z){}}
class EditorHoldNoteEvent:EditorEvent{public void SetHoldStartHeight(double h){}}
class Spacing{public bool Negative;public double GetY(EditorEvent e,double y)=>e.Y;public bool IsScrollRateNegative()=>Negative;public void UpdatePpsAndPpr(EditorEvent e,double i,double z){}}
class Layout{public List<int> Calls=new();public void PositionEvent(EditorEvent e,double y)=>Calls.Add(e.Id);}
class Host{public bool DrawMisc;public Spacing SpacingHelper=new();public Layout MiscEventWidgetLayoutManager=new();bool ShouldDrawMiscEvents()=>DrawMisc;bool IsBeyondVisibleFarEdge(double y,double h,double d)=>(y-h)*d>0;
void AddRegion(IChartRegion r,bool b,ref List<IChartRegion>x,ref HashSet<IChartRegion>y,double p,EditorEvent n,double a,double w){}void UpdateRegions(ref List<IChartRegion>x,ref HashSet<IChartRegion>y,double p,EditorEvent n,EditorEvent v){}void UpdateBeatMarkers(EditorEvent r,ref int x,ref int y,EditorEvent n,double a,double z,double p,double h,int m){}void CheckForCompletingHolds(HashSet<EditorHoldNoteEvent> h,double p,EditorEvent n){}bool CheckForCompletingHold(EditorHoldNoteEvent h,double p,EditorEvent n)=>true;
public object Run(string name,int max,bool draw,int pre,int[] kinds,bool negative=false,double[] ys=null){DrawMisc=draw;SpacingHelper.Negative=negative;MiscEventWidgetLayoutManager.Calls.Clear();var input=kinds.Select((kind,id)=>kind==3?(EditorEvent)new EditorHoldNoteEvent{Id=id,Lane=true,Y=id*10}:new EditorEvent{Id=id,Lane=kind==0,Misc=kind==1||kind==2,Y=kind==4?1000:id*10}).ToList();if(ys!=null)for(int i=0;i<input.Count;i++)input[i].Y=ys[i];var enumerator=input.GetEnumerator();var rateEnumerator=input.Where((e,id)=>kinds[id]==2).GetEnumerator();bool hasNextRateEvent=rateEnumerator.MoveNext();EditorEvent nextRateEvent=hasNextRateEvent?rateEnumerator.Current:null,rateEvent=null;var noteEvents=Enumerable.Range(1,pre).Select(i=>new EditorEvent{Id=-i,Lane=true}).ToList();var holdsNeedingToBeCompleted=new HashSet<EditorHoldNoteEvent>();var regionsNeedingToBeAdded=new List<IChartRegion>();var addedRegions=new HashSet<IChartRegion>();double previousRateEventY=0,interpolatedScrollRate=1,spacingZoom=1,startPosX=0,arrowW=64,arrowH=64,sizeZoom=1,screenHeight=200,directionSign=1;float miscEventAlpha=1,noteAlpha=1;int beatMarkerRow=0,beatMarkerLastRecordedRow=-1,numArrows=4,maxMarkersToDraw=256,numRateAlteringEventsProcessed=1,maxEventsToDraw=max;bool reachedEndOfScreen=false;
'''+body+r'''
return new{name,max,draw,pre,kinds,negative,ys,admitted=noteEvents.Select(e=>e.Id),layout=MiscEventWidgetLayoutManager.Calls.ToArray(),rates=numRateAlteringEventsProcessed,reachedEndOfScreen};}}
class Program{static void Main(){var output=new List<object>();var h=new Host();var sequences=new[]{new[]{0,1,0,1,0},new[]{2,2,0,1,2,0},new[]{0,2,2,2,1,0},new[]{1,1,0,0},new[]{2,2,2},new[]{3,1,0,2},new[]{4,0,1}};int n=0;foreach(var seq in sequences)foreach(var max in new[]{-1,0,1,2,3})foreach(var misc in new[]{false,true})foreach(var pre in new[]{0,2})foreach(var negative in new[]{false,true})output.Add(h.Run((n++).ToString(),max,misc,pre,seq,negative));foreach(var max in new[]{-1,0,1,2,3,4,5,6,10})foreach(var misc in new[]{false,true})output.Add(h.Run("native-"+max+"-"+misc,max,misc,0,new[]{1,2,0,1,0,2,0,0},false,new double[]{0,0,24,36,48,48,72,240}));Console.WriteLine(JsonSerializer.Serialize(output));}}
'''
with tempfile.TemporaryDirectory(prefix='editor-admission-') as d:
 d=Path(d);(d/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>false</EnableDefaultCompileItems><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Compile Include="Program.cs" /></ItemGroup></Project>');(d/'Program.cs').write_text(host)
 q=subprocess.run([str(a.dotnet),'run','--project',str(d/'Oracle.csproj'),'-c','Release','--verbosity','quiet'],capture_output=True,text=True)
 if q.returncode:raise RuntimeError(q.stdout+q.stderr)
 cases=json.loads(q.stdout[q.stdout.index('[{"'):]);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(cases,indent=2)+'\n')
 a.output.with_suffix('.reference.json').write_text(json.dumps({'pin':pin,'source':'StepManiaEditor/ActiveEditorChart.cs','sha256':hashlib.sha256(s.encode()).hexdigest(),'method':'UpdateChartEvents unchanged scan-forward while block','cases':len(cases),'host':['pre-discovered holds','ordered events/rates','Y coordinates and dimensions','region/beat/hold callbacks no-op'],'not_proved':['start-row search','hold discovery','source full event comparer','native rendering']},indent=2)+'\n');print(len(cases),'source admission cases')
