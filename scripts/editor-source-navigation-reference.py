#!/usr/bin/env python3
"""Execute unchanged pinned navigation methods with explicit chart/tree adapters."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['editor-root','dotnet','output']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2'
def source(rel):return subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/'+rel],text=True,encoding='utf-8-sig')
def method(s,sig):
 start=s.index(sig);end=s.index('{',start)+1;depth=1
 while depth:depth+=(s[end]=='{')-(s[end]=='}');end+=1
 return s[start:end]
e=source('Editor.cs');c=source('EditorChart.cs');methods=[method(e,'private void OnMoveToPreviousMeasure()'),method(e,'private void OnMoveToNextMeasure()'),method(e,'private void MoveToChartPosition(double chartPosition)'),method(c,'public double GetEndPosition()')]
program=r'''using System;using System.Collections.Generic;using System.Text.Json;
class PrefScroll{public bool StopPlaybackWhenScrolling;}class Preferences{public static Preferences Instance=new();public PrefScroll PreferencesScroll=new();}
class Signature{public int N=4,D=4;public int GetNumerator()=>N;public int GetDenominator()=>D;}
class Rate{public Signature Sig=new();public Signature GetTimeSignature()=>Sig;}
class Rates{public Rate Value=new();public Rate FindActiveRateAlteringEventForPosition(double p)=>Value;}
class Position{public double ChartPosition;}
class Event{public int End;public virtual int GetEndRow()=>End;}class EditorPreviewRegionEvent:Event{}
class Enumerator{public List<Event> Events;int i;public Enumerator(List<Event> events){Events=events;i=events.Count;}public Event Current=>Events[i];public bool MoveNext(){if(i==Events.Count)i--;return i>=0;}public bool MovePrev(){i--;return i>=0;}}
class Tree{public List<Event> Events=new();public Enumerator Last()=>new(Events);}
class Chart{public Rates Rates=new();public Tree EditorEvents=new();public Rates GetRateAlteringEvents()=>Rates;__END__}
class Editor{const int MaxValidDenominator=48,NumBeatsPerMeasure=4;public Chart FocusedChart=new();public Position Position=new();public bool Stopped;Position GetPosition()=>Position;void StopPlayback()=>Stopped=true;void SetChartPosition(double p)=>Position.ChartPosition=p;void UpdateAutoPlayFromScrolling(){}public void Run(bool prev){if(prev)OnMoveToPreviousMeasure();else OnMoveToNextMeasure();}__MOVE____PREV____NEXT__}
class Program{static void Main(){var measures=new List<object>();foreach(var row in new[]{-.5,.5,192.5})foreach(var previous in new[]{false,true})foreach(var stop in new[]{false,true}){Preferences.Instance.PreferencesScroll.StopPlaybackWhenScrolling=stop;var e=new Editor();e.Position.ChartPosition=row;e.Run(previous);measures.Add(new{row,previous,stop,destination=e.Position.ChartPosition,stopped=e.Stopped});}var ends=new List<object>();foreach(var values in new[]{new[]{384},new[]{576},new[]{4800,384},new[]{384,10000}}){var chart=new Chart();for(int i=0;i<values.Length;i++)chart.EditorEvents.Events.Add(i==1&&values[i]==10000?new EditorPreviewRegionEvent{End=values[i]}:new Event{End=values[i]});ends.Add(new{values,result=chart.GetEndPosition()});}Console.Write(JsonSerializer.Serialize(new{measures,ends}));}}
'''.replace('__PREV__',methods[0]).replace('__NEXT__',methods[1]).replace('__MOVE__',methods[2]).replace('__END__',methods[3])
with tempfile.TemporaryDirectory(prefix='source-navigation-oracle-')as temp:
 d=Path(temp);(d/'Program.cs').write_text(program);(d/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
 r=subprocess.run([str(a.dotnet),'build',str(d/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],capture_output=True,text=True)
 if r.returncode:raise SystemExit(r.stdout+r.stderr)
 data=json.loads(subprocess.check_output([str(a.dotnet),str(d/'bin/Release/net10.0/Oracle.dll')],text=True))
 a.output.write_text(json.dumps({'source':pin,'method_sha256':[hashlib.sha256(x.encode()).hexdigest()for x in methods],'adapters':'signature fixed 4/4; tree events provide prescribed GetEndRow; source methods unchanged','result':data},indent=2)+'\n')
 print('12 original measure/policy cases and4 original chart-end traversal cases executed')
