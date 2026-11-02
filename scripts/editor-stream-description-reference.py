#!/usr/bin/env python3
"""Run original Use Stream callback/property with the real Fumen undo queue."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path

PIN='48e18f71655d0f0d86144c63239331833501fda2'
FP='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser()
p.add_argument('--source',type=Path,required=True)
p.add_argument('--reference-root',type=Path,required=True)
p.add_argument('--dotnet',required=True)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args()
def original(path):
    return subprocess.check_output(['git','-C',str(a.source),'show',PIN+':StepManiaEditor/'+path]).decode('utf-8-sig')
def member(text,marker):
    start=text.index(marker);brace=text.index('{',start);end=brace+1;depth=1
    while depth:
        depth+=(text[end]=='{')-(text[end]=='}');end+=1
    return text[start:end]
callback=member(original('UI/UIChartProperties.cs'),'private static void CopyChartStreamToDescription(')
prop=member(original('EditorChart.cs'),'public string Description')
fumen=a.reference_root/'Fumen'
assert subprocess.check_output(['git','-C',str(fumen),'rev-parse','HEAD'],text=True).strip()==FP
dll=fumen/'Fumen/bin/Release/net10.0/Fumen.dll'
program='''using System;using System.Linq;using System.Collections.Generic;using System.Text.Json;using Fumen;using static System.Diagnostics.Debug;
class EditorChart {private string DescriptionInternal;public const string NotificationDescriptionChanged="description";
public string Stream;public bool CanBeEdited()=>true;private void Notify(string s,object value){}
public string GetStreamBreakdown()=>Stream;
'''+prop+'''}
class UIChartProperties {'''+callback+'''public static void Apply(EditorChart chart)=>CopyChartStreamToDescription(chart);}
class Program {static void Main(){var cases=new List<object>();
foreach(var pair in new[]{("manual",""),("",""),("manual","2"),("2","2"),(" ","4-8"),("old","4 (2) 8")}){
ActionQueue.Instance.Clear();var chart=new EditorChart{Description=pair.Item1,Stream=pair.Item2};
UIChartProperties.Apply(chart);var after=chart.Description;var undo=ActionQueue.Instance.Undo();var undone=chart.Description;
var redo=ActionQueue.Instance.Redo();cases.Add(new{before=pair.Item1,stream=pair.Item2,after,undone,redone=chart.Description,hadUndo=undo!=null,hadRedo=redo!=null});
}Console.Write(JsonSerializer.Serialize(cases));}}
'''
with tempfile.TemporaryDirectory(prefix='editor-stream-description-') as directory:
    root=Path(directory);(root/'Program.cs').write_text(program)
    (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
    built=subprocess.run([a.dotnet,'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],capture_output=True,text=True)
    if built.returncode:raise SystemExit(built.stdout+built.stderr)
    assembly=root/'bin/Release/net10.0/Oracle.dll'
    result=json.loads(subprocess.check_output([a.dotnet,str(assembly)],text=True))
    assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n')
a.output.with_suffix('.reference.json').write_text(json.dumps({
    'editor_commit':PIN,'fumen_commit':FP,'callback_sha256':hashlib.sha256(callback.encode()).hexdigest(),
    'property_sha256':hashlib.sha256(prop.encode()).hexdigest(),'program_sha256':hashlib.sha256(program.encode()).hexdigest(),
    'assembly_sha256':assembly_hash,'fumen_dll_sha256':hashlib.sha256(dll.read_bytes()).hexdigest(),
    'scope':'Complete original CopyChartStreamToDescription and Description property, real Fumen reflection action/Undo/Redo. Stream calculation and notification host are explicit adapters; stream calculation has its separate original density oracle.'
},indent=2)+'\n')
print('Executed',len(result),'original Use Stream history cases')
