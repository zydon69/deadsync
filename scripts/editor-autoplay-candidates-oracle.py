#!/usr/bin/env python3
"""Run original AutoPlayer candidate search, chart hold lookup and event-tree windows.
Timing/fake flags and event tie order are explicit adapter snapshots, not a C# timing oracle.
"""
import argparse,hashlib,json,subprocess,tempfile,shutil
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def extract(source,marker):
    start=source.index(marker);brace=source.index('{',start);end=brace+1;depth=1
    while depth:depth+=(source[end]=='{')-(source[end]=='}');end+=1
    return source[start:end]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--fumen-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=a.reference_root.resolve()
    commit=subprocess.check_output(['git','-C',str(r),'rev-parse','HEAD'],text=True).strip()
    if commit!=PIN:raise SystemExit('Wrong reference commit')
    paths={k:r/'StepManiaEditor'/v for k,v in dict(autoplay='AutoPlayer.cs',chart='EditorChart.cs',tree='EditorEvents/Containers/EventTree.cs',utils='EditorEvents/Containers/EventTreeUtils.cs').items()}
    for path in paths.values():
        pinned=subprocess.check_output(['git','-C',str(r),'show',f'{PIN}:{path.relative_to(r)}'])
        if path.read_bytes()!=pinned:raise SystemExit(f'{path} differs from pinned git source')
    s={k:v.read_text(encoding='utf-8-sig') for k,v in paths.items()}
    parts={k:extract(s[f],marker) for k,f,marker in [('next','autoplay','private Event[] GetNextInputs('),('holds','chart','public EditorHoldNoteEvent[] GetHoldsOverlappingTime('),('best','tree','public IReadOnlyRedBlackTree<EditorEvent>.IReadOnlyRedBlackTreeEnumerator FindBestByTime('),('before','tree','public IReadOnlyRedBlackTree<EditorEvent>.IReadOnlyRedBlackTreeEnumerator FindGreatestAtOrBeforeChartTime('),('ensure_before','utils','public static bool EnsureGreatestLessThanTime<T>('),('ensure_before_equal','utils','public static bool EnsureGreatestLessThanOrEqualToTime<T>('),('ensure_after_equal','utils','public static bool EnsureLeastGreaterThanOrEqualToTime<T>('),('unset','utils','public static bool UnsetAndReturnIfWasValid<T>(')]}
    def note(lane,row,time,kind='Tap',end=None,fake=False,player=0):return dict(lane=lane,row=row,time=time,kind=kind,end=end,fake=fake,player=player)
    cases=[dict(name='fake-region-and-mid-hold',timing='#FAKES:1=1;',notes='20000000\n00000000\n30000000\n00000000\n&\n00000000\n01000000\n00L00000\n00000000',events=[note(0,0,0,'Hold',1),note(1,48,.5,fake=True,player=1),note(2,96,1,'Lift',player=1)]),dict(name='warp-equal-head-times',timing='#WARPS:1=1;',notes='00200000\n10000000\n01000000\n00000000,\n00300000',events=[note(2,0,0,'Hold',1.5),note(0,48,.5,fake=True),note(1,96,.5)]),dict(name='negative-stop-fake-head',timing='#STOPS:1=-0.5;',notes='00000000\n10000000\n01000000\n00000000',events=[note(0,48,.5,fake=True),note(1,96,.5)]),dict(name='fake-and-mine-block-older-owner-hold',timing='',notes='20000000\n00000000\n00000000\n30000000\n&\n00000000\nM0000000\n00000000\n00000000',events=[note(0,0,0,'Hold',1.5),note(0,48,.5,'Mine',player=1)]),dict(name='same-lane-owner-equality',timing='',notes='20000000\n00000000\n30000000\n00000000\n&\n10000000\n00000000\n00000000\n00000000',events=[note(0,0,0,'Tap',player=1),note(0,0,0,'Hold',1)])]
    for tag in ['#LABELS:0.5=middle;','#SAMPLESTART:0.25;#SAMPLELENGTH:0.5;','#LASTSECONDHINT:0.25;']:
        cases.append(dict(name='nonlane-predecessor-'+tag.split(':')[0][1:],timing=tag,notes='20000000\n00000000\n00000000\n30000000\n&\n00000000\nM0000000\n00000000\n00000000',events=[note(0,0,0,'Hold',1.5),note(-1,24,.25,'Marker'),note(0,48,.5,'Mine',player=1)]))
    cases.append(dict(name='hint-before-equal-row-hold-predecessor',timing='#LASTSECONDHINT:0.5;',notes='00000000\n20000000\n00000000\n00000000,\n30000000\n&\n00000000\n00000000\nM0000000\n00000000',events=[note(-1,48,.5,'Marker'),note(0,48,.5,'Hold',2),note(0,96,1,'Mine',player=1)]))
    for c in cases:c['queries']=[-.01,0,.25,.5,.5000000001,.75,1,1.0000000001,1.5,2]
    program='''using System;using System.Collections.Generic;using System.Text.Json;using Fumen;
class Event{public double TimeSeconds;}class LaneTapNote:Event{}class LaneHoldStartNote:Event{}class LaneHoldEndNote:Event{}
interface IReadOnlyEditorPosition{double ChartTime{get;}}record Position(double ChartTime):IReadOnlyEditorPosition;
static class StepManiaLibrary {public static class Constants {public const int InvalidArrowIndex=-1;}}
class EventConfig {public static EditorEvent CreateSearchEventConfigWithOnlyTime(EditorChart c,double time)=>new(){Time=time,Search=true};}
class EditorEvent:IComparable<EditorEvent>{public int Lane,Order,Player;public double Time;public double? End;public bool Fake,Search;public string Kind;
public static EditorEvent CreateEvent(EditorEvent c)=>c;public int CompareTo(EditorEvent e){int t=Time.CompareTo(e.Time);return t!=0?t:(Search||e.Search?0:Order.CompareTo(e.Order));}
public int GetLane()=>Lane;public double GetChartTime()=>Time;public double GetEndChartTime()=>End??Time;public bool IsFake()=>Fake;public bool IsBeingEdited()=>false;
public Event GetEvent(){Event e=Kind=="Hold"?new LaneHoldStartNote():new LaneTapNote();e.TimeSeconds=Time;return e;}public Event GetAdditionalEvent()=>new LaneHoldEndNote(){TimeSeconds=End.Value};}
class EditorTapNoteEvent:EditorEvent{}class EditorLiftNoteEvent:EditorEvent{}class EditorHoldNoteEvent:EditorEvent{public Event GetHoldEndEvent()=>GetAdditionalEvent();}
class EventTree:RedBlackTree<EditorEvent>{EditorChart Chart;public EventTree(EditorChart c){Chart=c;}
'''+parts['best']+parts['before']+parts['ensure_before']+parts['ensure_before_equal']+parts['ensure_after_equal']+parts['unset']+'''}
class EditorChart{public int NumInputs=8;public EventTree EditorEvents;public EditorChart(){EditorEvents=new(this);}public EventTree GetEvents()=>EditorEvents;
'''+parts['holds']+'''}
class Candidate{EditorChart ActiveChart;public Candidate(EditorChart c){ActiveChart=c;}
'''+parts['next']+'''
public object Run(double time){var inputs=GetNextInputs(new Position(time));return Array.ConvertAll(inputs,e=>e==null?null:(object)new{kind=e is LaneHoldEndNote?"HoldEnd":e is LaneHoldStartNote?"HoldStart":"Tap",time=e.TimeSeconds});}
static void Main(){var requests=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var results=new List<object>();foreach(var c in requests.EnumerateArray()){var chart=new EditorChart();int ordinal=0;foreach(var n in c.GetProperty("events").EnumerateArray()){string kind=n.GetProperty("kind").GetString();EditorEvent e=kind switch{"Hold"=>new EditorHoldNoteEvent(),"Lift"=>new EditorLiftNoteEvent(),"Tap"=>new EditorTapNoteEvent(),_=>new EditorEvent()};e.Kind=kind;e.Lane=n.GetProperty("lane").GetInt32();e.Order=ordinal++;e.Time=n.GetProperty("time").GetDouble();e.Fake=n.GetProperty("fake").GetBoolean();if(n.GetProperty("end").ValueKind!=JsonValueKind.Null)e.End=n.GetProperty("end").GetDouble();chart.EditorEvents.Insert(e);}var candidate=new Candidate(chart);var values=new List<object>();foreach(var t in c.GetProperty("queries").EnumerateArray())values.Add(candidate.Run(t.GetDouble()));results.Add(values);}Console.Write(JsonSerializer.Serialize(results));}}
'''
    fumen=a.fumen_root/'Fumen/RedBlackTree.cs'
    fumen_commit=subprocess.check_output(['git','-C',str(a.fumen_root),'rev-parse','HEAD'],text=True).strip()
    pinned_tree=subprocess.check_output(['git','-C',str(a.fumen_root),'show',f'{fumen_commit}:Fumen/RedBlackTree.cs'])
    if fumen.read_bytes()!=pinned_tree:raise SystemExit('Fumen tree differs from its recorded git source')
    with tempfile.TemporaryDirectory(prefix='deadsync-autoplay-candidate-') as folder:
        folder=Path(folder);(folder/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(folder/'Program.cs').write_text(program);shutil.copyfile(fumen,folder/'RedBlackTree.cs')
        build=subprocess.run([str(a.dotnet),'build',str(folder/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
        if build.returncode:raise SystemExit(build.stdout+build.stderr)
        result=subprocess.run([str(a.dotnet),str(folder/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),capture_output=True,text=True,check=True)
    expected=json.loads(result.stdout)
    doc=dict(reference_commit=commit,source_bytes_verified_against_git=True,fumen_commit=fumen_commit,source_sha256={str(v.relative_to(r)):hashlib.sha256(v.read_bytes()).hexdigest() for v in paths.values()},fumen_tree_sha256=hashlib.sha256(fumen.read_bytes()).hexdigest(),method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in parts.items()},scope='Original GetNextInputs, GetHoldsOverlappingTime, time-window search/Ensure methods and unchanged Fumen RedBlackTree. Adapted event time/fake/tie-order snapshots: not original timing, complete EditorEvent comparer, or rendering.',cases=[dict(**c,expected=e) for c,e in zip(cases,expected)])
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(doc,indent=2)+'\n');print(f'Generated {len(cases)} candidate sequences / {sum(len(c["queries"]) for c in cases)} chart-time queries')
if __name__=='__main__':main()
