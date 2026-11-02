#!/usr/bin/env python3
"""Complete original chart string setters and real Fumen undo queue."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2';FP='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def block(text,marker):
 start=text.index(marker);opening=text.index('{',start);end=opening+1;depth=1
 while depth:depth+=(text[end]=='{')-(text[end]=='}');end+=1
 return text[start:end]
p=argparse.ArgumentParser();p.add_argument('--source',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
def original(path):return subprocess.check_output(['git','-C',str(a.source),'show',f'{PIN}:{path}']).decode('utf-8-sig')
chart=original('StepManiaEditor/EditorChart.cs');properties=[block(chart,'public string '+name) for name in ['Name','Description','Style','Credit','MusicPath']]
fumen=a.reference_root/'Fumen';assert subprocess.check_output(['git','-C',str(fumen),'rev-parse','HEAD'],text=True).strip()==FP;dll=fumen/'Fumen/bin/Release/net10.0/Fumen.dll'
program='''using System;using System.Collections.Generic;using System.Text.Json;using Fumen;using static System.Diagnostics.Debug;
class EditorChart {public bool CanBeEdited()=>true;private string NameInternal,DescriptionInternal,StyleInternal,CreditInternal,MusicPathInternal;private const string NotificationNameChanged="name",NotificationDescriptionChanged="description",NotificationMusicChanged="music";private void Notify(string name,object sender){}
'''+'\n'.join(properties)+'''}
class Program {static void Main(){var rows=new List<object>();foreach(var format in new[]{"sm","ssc"})foreach(var field in new[]{"Name","Description","Style","Credit","MusicPath"})foreach(var value in new[]{""," ","  authored  ","Été : ; // 日本語"}){ActionQueue.Instance.Clear();var chart=new EditorChart();var property=typeof(EditorChart).GetProperty(field);property.SetValue(chart,"before");ActionQueue.Instance.Do(new ActionSetObjectFieldOrPropertyReference<string>(chart,field,value,true));var after=property.GetValue(chart);var undo=ActionQueue.Instance.Undo()!=null;var undone=property.GetValue(chart);var redo=ActionQueue.Instance.Redo()!=null;var redone=property.GetValue(chart);rows.Add(new{format,field,value,after,undone,redone,undo,redo});}Console.Write(JsonSerializer.Serialize(rows));}}
'''
with tempfile.TemporaryDirectory(prefix='source-chart-authoring-')as temp:
 root=Path(temp);(root/'Program.cs').write_text(program);(root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>');r=subprocess.run([a.dotnet,'run','--project',str(root/'Oracle.csproj'),'--verbosity','quiet'],capture_output=True,text=True,check=True);result=json.loads(next(line for line in r.stdout.splitlines()if line.startswith('[')))
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':PIN,'FumenCommit':FP,'PropertySha256':[hashlib.sha256(s.encode()).hexdigest()for s in properties],'HostSha256':hashlib.sha256(program.encode()).hexdigest(),'DllSha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'Scope':'Five complete original EditorChart setters; actual Fumen reflection string action + undo/redo queue. Host editability always true and notifications stubbed. Format labels demonstrate the setters have no format gate; no claim whole original EditorChart or writers executes.'},indent=2)+'\n');print('Executed',len(result),'original chart property action/Undo/Redo cases.')
