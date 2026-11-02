#!/usr/bin/env python3
"""Execute whole pinned Stream preferences, converter, notifier and Undo/Redo action."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser()
for n in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+n,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for path,sha in [(a.editor_root,pin),(a.reference_root/'Fumen',fp)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==sha
sources={n:subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{pin}:StepManiaEditor/'+n]) for n in ['Preferences/PreferencesStream.cs','SubdivisionType.cs']}
inputs=[{}, {'NoteType':'QuarterNotes','AccumulationType':'Row','ShowBreakLengths':True,'MinimumLengthToConsiderStream':8,'ShortBreakCutoff':64,'ShortBreakCharacter':'/','LongBreakCharacter':':'}, {'NoteType':'Future','AccumulationType':'Future'}, {'NoteType':999,'AccumulationType':999}, {'MinimumLengthToConsiderStream':-3,'ShortBreakCutoff':-4,'ShortBreakCharacter':'é','LongBreakCharacter':' '}, {'ShowStreamPreferencesWindow':True,'MinimumLengthToConsiderStream':2147483647}]
inputs += [{'NoteType': 'eighthnotetriplets', 'AccumulationType': 'row'}, {'NoteType': '2', 'AccumulationType': '1'}, {'NoteType': 2, 'AccumulationType': 1}, {'ShortBreakCharacter':'\t','LongBreakCharacter':'\n'}]
adapter='''using System.Text.Json;using Fumen;using Fumen.Converters;using StepManiaEditor;
namespace StepManiaEditor {class Preferences {public static Preferences Instance=new();public PreferencesStream PreferencesStream=new();}}
class Program {static void Main(string[] args){var factory=new PermissiveEnumJsonConverterFactory();PreferencesStream.RegisterDefaultsForInvalidEnumValues(factory);var options=new JsonSerializerOptions{IncludeFields=true,Converters={factory}};var cases=new List<object>();foreach(var json in JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement.EnumerateArray()){var p=JsonSerializer.Deserialize<PreferencesStream>(json.GetRawText(),options);Preferences.Instance.PreferencesStream=p;var normalized=JsonSerializer.SerializeToElement(p,options);p.NoteType=SubdivisionType.EighthNoteTriplets;p.AccumulationType=StepAccumulationType.Row;p.ShowBreakLengths=true;p.MinimumLengthToConsiderStream=3;p.ShortBreakCutoff=11;p.ShortBreakCharacter='é';p.LongBreakCharacter=':';var changed=JsonSerializer.SerializeToElement(p,options);p.RestoreDefaults();var restored=JsonSerializer.SerializeToElement(p,options);ActionQueue.Instance.Undo();var undone=JsonSerializer.SerializeToElement(p,options);ActionQueue.Instance.Redo();var redone=JsonSerializer.SerializeToElement(p,options);cases.Add(new{Input=json.Clone(),Normalized=normalized,Changed=changed,Restored=restored,Undone=undone,Redone=redone});ActionQueue.Instance.Clear();}Console.Write(JsonSerializer.Serialize(cases));}}
'''
with tempfile.TemporaryDirectory(prefix='deadsync-stream-source-') as d:
 t=Path(d)
 for n,data in sources.items():(t/Path(n).name).write_text(data.decode('utf-8-sig'))
 (t/'Program.cs').write_text(adapter);(t/'request.json').write_text(json.dumps(inputs))
 dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
 (t/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
 assembly=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly),str(t/'request.json')],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fp,'SourceSha256':{n:hashlib.sha256(v).hexdigest() for n,v in sources.items()},'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':sha,'FumenAssemblySha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'Scope':'Whole unchanged PreferencesStream/SubdivisionType, real Fumen notifier/converter/Undo queue. No PostLoad method exists in source. UI immediate editing not executed.'},indent=2)+'\n');print('Executed',len(result),'source stream preferences/history cases.')
