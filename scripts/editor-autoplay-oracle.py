#!/usr/bin/env python3
"""Execute unchanged pinned AutoPlayer transition methods against next-input snapshots.
Candidate search, editor timing, rendering and animation are deliberately outside this oracle.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
PIN = '48e18f71655d0f0d86144c63239331833501fda2'
def extract(source, marker):
    start = source.index(marker); brace = source.index('{', start); end = brace + 1; depth = 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}'); end += 1
    return source[start:end]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    root=a.reference_root.resolve(); commit=subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()
    if commit != PIN: raise SystemExit(f'Reference must be {PIN}, found {commit}')
    path=root/'StepManiaEditor/AutoPlayer.cs'
    pinned=subprocess.check_output(['git','-C',str(root),'show',f'{PIN}:StepManiaEditor/AutoPlayer.cs'])
    if path.read_bytes()!=pinned: raise SystemExit('AutoPlayer.cs differs from pinned git source')
    source=path.read_text(encoding='utf-8-sig')
    parts={name:extract(source,marker) for name,marker in [('state','private sealed class AutoPlayState'),('update','public void Update(IReadOnlyEditorPosition position)'),('stop','public void Stop()')]}
    cases=[]
    for kind in ['Tap','HoldStart','HoldEnd']:
        cases.append(dict(name=f'{kind}-normal',steps=[dict(time=0,next=[dict(kind=kind,time=1)]),dict(time=1.01,next=[None]),dict(reset=True),dict(time=1.1,next=[dict(kind='HoldEnd',time=2)]),dict(time=2.1,next=[None])]))
    cases += [dict(name='warp-transitions',steps=[dict(time=0,next=[dict(kind='HoldStart',time=1)]),dict(time=1.1,next=[dict(kind='HoldEnd',time=2)]),dict(time=2.1,next=[dict(kind='HoldEnd',time=3)]),dict(time=3.1,next=[dict(kind='Tap',time=4)]),dict(time=4.1,next=[None])]),dict(name='equal-time-kind-and-ulp',steps=[dict(time=0,next=[dict(kind='Tap',time=1)]),dict(time=1,next=[dict(kind='HoldStart',time=1)]),dict(time=1,next=[dict(kind='HoldEnd',time=1.0000000000000002)]),dict(time=1.000000000000001,next=[dict(kind='HoldEnd',time=1.0000000000000004)]),dict(time=2,next=[None])]),dict(name='zero-negative-seek-reset',steps=[dict(time=-1,next=[dict(kind='HoldEnd',time=0)]),dict(time=0,next=[None]),dict(reset=True),dict(time=-2,next=[dict(kind='Tap',time=-1)]),dict(time=-.5,next=[None])])]
    # Several lanes exercise callback ordering and independent held state.
    cases.append(dict(name='three-lanes',steps=[dict(time=0,next=[dict(kind='Tap',time=1),dict(kind='HoldEnd',time=2),None]),dict(time=1.1,next=[None,dict(kind='HoldEnd',time=2),dict(kind='HoldStart',time=2)]),dict(time=2.1,next=[None,None,dict(kind='HoldEnd',time=3)]),dict(reset=True)]))
    program='''using System;using System.Collections.Generic;using System.Text.Json;
class Event {public double TimeSeconds;}
class LaneTapNote:Event{} class LaneHoldStartNote:Event{} class LaneHoldEndNote:Event{}
interface IReadOnlyEditorPosition {double ChartTime{get;}}
record Position(double ChartTime):IReadOnlyEditorPosition;
static class Equality {public static bool DoubleEquals(this double a,double b) {long x=BitConverter.DoubleToInt64Bits(a),y=BitConverter.DoubleToInt64Bits(b);return (x<0)!=(y<0)?a==b:Math.Abs(x-y)<=1;}}
class Receptor {public bool Held; public int Lane; public List<object> Actions; public bool IsAutoplayHeld()=>Held; public void OnAutoplayInputDown(double d=0){Held=true;Actions.Add(new {lane=Lane,kind="Down",time_delta=d});} public void OnAutoplayInputUp(double d=0){Held=false;Actions.Add(new {lane=Lane,kind="Up",time_delta=d});} public void OnAutoplayInputCancel(){Held=false;Actions.Add(new {lane=Lane,kind="Cancel",time_delta=0.0});}}
class Reference {
'''+parts['state']+'''
class Chart {public int NumInputs;} Chart ActiveChart=new();AutoPlayState[] AutoPlayStates;Receptor[] Receptors;Event[] Next;List<object> Actions=new();
public Reference(int lanes){ActiveChart.NumInputs=lanes;AutoPlayStates=new AutoPlayState[lanes];Receptors=new Receptor[lanes];for(int i=0;i<lanes;i++){AutoPlayStates[i]=new();Receptors[i]=new(){Lane=i,Actions=Actions};}}
Event[] GetNextInputs(IReadOnlyEditorPosition p)=>Next;
'''+parts['update']+parts['stop']+'''
public object Run(JsonElement step){Actions.Clear();if(step.TryGetProperty("reset",out var reset)&&reset.GetBoolean())Stop();else{var list=new List<Event>();foreach(var input in step.GetProperty("next").EnumerateArray()){Event e=null;if(input.ValueKind!=JsonValueKind.Null){e=input.GetProperty("kind").GetString() switch {"HoldStart"=>new LaneHoldStartNote(),"HoldEnd"=>new LaneHoldEndNote(),_=>new LaneTapNote()};e.TimeSeconds=input.GetProperty("time").GetDouble();}list.Add(e);}Next=list.ToArray();Update(new Position(step.GetProperty("time").GetDouble()));}return new {actions=Actions.ToArray(),held=Array.ConvertAll(Receptors,r=>r.Held)};}
static void Main(){var cases=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var results=new List<object>();foreach(var c in cases.EnumerateArray()){int lanes=c.GetProperty("steps")[0].GetProperty("next").GetArrayLength();var r=new Reference(lanes);var stages=new List<object>();foreach(var step in c.GetProperty("steps").EnumerateArray())stages.Add(r.Run(step));results.Add(stages);}Console.Write(JsonSerializer.Serialize(results));}}
'''
    with tempfile.TemporaryDirectory(prefix='deadsync-autoplay-oracle-') as folder:
        folder=Path(folder);(folder/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>');(folder/'Program.cs').write_text(program)
        build=subprocess.run([str(a.dotnet),'build',str(folder/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,text=True)
        if build.returncode: raise SystemExit(build.stdout+build.stderr)
        result=subprocess.run([str(a.dotnet),str(folder/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),text=True,capture_output=True,check=True)
    expected=json.loads(result.stdout)
    document=dict(reference_commit=commit,source_bytes_verified_against_git=True,source='StepManiaEditor/AutoPlayer.cs',source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),method_sha256={k:hashlib.sha256(v.encode()).hexdigest() for k,v in parts.items()},scope='Unchanged AutoPlayState, Update and Stop methods; supplied next-input snapshots. Excludes candidate search, Fumen/editor timing, rendering and animations.',cases=[dict(**c,expected=e) for c,e in zip(cases,expected)])
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(document,indent=2)+'\n');print(f'Generated {len(cases)} transition sequences from {commit}')
if __name__=='__main__': main()
