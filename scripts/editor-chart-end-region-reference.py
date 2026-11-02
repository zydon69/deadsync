#!/usr/bin/env python3
"""Real pinned Fumen AddWarps/AddFakeSegmentEvents plus unchanged editor constructors/end methods."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for k in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+k,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
assert subprocess.check_output(['git','-C',str(a.reference_root/'Fumen'),'rev-parse','HEAD'],text=True).strip()==fp
methods=[]
def extract(s,sig):
 i=s.index(sig);j=s.index('{',i)+1;depth=1
 while depth:depth+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[i:j]
classes=[]
for name,event,const in [('EditorWarpEvent','Warp','MinLength'),('EditorFakeSegmentEvent','FakeSegment','MinFakeSegmentLength')]:
 src=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/EditorEvents/'+name+'.cs'],text=True,encoding='utf-8-sig')
 ctor=extract(src,f'public {name}(EventConfig config, {event} chartEvent)');end=extract(src,'public override int GetEndRow()');methods.extend([ctor,end]);field=event+'Event'
 classes.append(f'class {name}:EditorEvent{{public const int {const}=1;private readonly {event} {field};private bool WidthDirty;'+ctor+end+'}')
program=r'''using System;using System.Collections.Generic;using System.Text.Json;using Fumen.ChartDefinition;using Fumen.Converters;using static System.Diagnostics.Debug;
class EventConfig{public int Row;}
class EditorEvent{int Row;public EditorEvent(EventConfig c){Row=c.Row;}public int GetRow()=>Row;public virtual int GetEndRow()=>Row;}
__CLASSES__
class Program{static void Main(){var cases=new List<object>();foreach(var kind in new[]{"warp","fake"})foreach(var beats in new[]{-1.0,0.0,0.000001,1.0/96,0.02,31.0/48,49.0/48,1.333333,3.14159}){var chart=new Chart();chart.Layers.Add(new Layer());var entries=new Dictionary<double,double>{{8,beats}};if(kind=="warp")SMCommon.AddWarps(entries,chart,null,false);else SMCommon.AddFakeSegmentEvents(entries,chart,null,false);int? rows=null,end=null;if(chart.Layers[0].Events.Count>0){var e=chart.Layers[0].Events[0];var config=new EventConfig{Row=e.IntegerPosition};if(e is Warp w){rows=w.LengthIntegerPosition;end=new EditorWarpEvent(config,w).GetEndRow();}else if(e is FakeSegment f){rows=f.LengthIntegerPosition;end=new EditorFakeSegmentEvent(config,f).GetEndRow();}}cases.Add(new{kind,beats,reader_length=rows,end});}Console.Write(JsonSerializer.Serialize(cases));}}
'''.replace('__CLASSES__','\n'.join(classes))
with tempfile.TemporaryDirectory(prefix='region-end-source-')as temp:
 d=Path(temp);(d/'Program.cs').write_text(program);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(d/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
 r=subprocess.run([str(a.dotnet),'build',str(d/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],capture_output=True,text=True)
 if r.returncode:raise SystemExit(r.stdout+r.stderr)
 cases=json.loads(subprocess.check_output([str(a.dotnet),str(d/'bin/Release/net10.0/Oracle.dll')],text=True));a.output.write_text(json.dumps({'source':pin,'fumen':fp,'method_sha256':[hashlib.sha256(s.encode()).hexdigest()for s in methods],'adapters':'EditorEvent base supplies actual Fumen event.IntegerPosition to GetRow; original editor constructors and GetEndRow unchanged; real Fumen AddWarps/AddFakeSegmentEvents','cases':cases},indent=2)+'\n');print(f'{len(cases)} real Fumen + original editor region-end cases executed')
