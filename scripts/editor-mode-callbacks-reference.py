#!/usr/bin/env python3
"""Execute complete pinned enum declarations and the two original Editor callbacks.
The whole Editor Game host cannot build standalone: only unrelated host dependencies
are stubbed; callback bodies and declarations are extracted byte-for-byte.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def declaration(text,marker):
 start=text.index(marker);opening=text.index('{',start);depth=1;end=opening+1
 while depth:
  depth+=(text[end]=='{')-(text[end]=='}');end+=1
 return text[start:end]
p=argparse.ArgumentParser();p.add_argument('--source',type=Path,required=True);p.add_argument('--dotnet',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
source=subprocess.check_output(['git','-C',str(a.source),'show',f'{PIN}:StepManiaEditor/Editor.cs']).decode('utf-8-sig')
assert (a.source/'StepManiaEditor/Editor.cs').read_text(encoding='utf-8-sig')==source
parts=[declaration(source,x) for x in ['public enum SpacingMode','public enum NoteEntryMode','private void OnToggleSpacingMode()','private void OnToggleNoteEntryMode()']]
host='''using System.Text.Json;
class Logger {public static void Info(string message){}}
class Preferences {public static Preferences Instance=new();public Editor.NoteEntryMode NoteEntryMode;public Scroll PreferencesScroll=new();public class Scroll {public Editor.SpacingMode SpacingMode;}}
partial class Editor {
'''+ '\n'.join(parts)+'''
public void Note(){OnToggleNoteEntryMode();}public void Spacing(){OnToggleSpacingMode();}}
class Program {static void Main(){var output=new List<object>();foreach(Editor.NoteEntryMode note in Enum.GetValues<Editor.NoteEntryMode>())foreach(Editor.SpacingMode spacing in Enum.GetValues<Editor.SpacingMode>()) {Preferences.Instance=new(){NoteEntryMode=note,PreferencesScroll=new(){SpacingMode=spacing}};var editor=new Editor();var frames=new List<object>();frames.Add(new{Operation="initial",Note=Preferences.Instance.NoteEntryMode.ToString(),Spacing=Preferences.Instance.PreferencesScroll.SpacingMode.ToString()});foreach(var operation in new[]{"note","spacing","spacing","note","spacing","note","spacing"}){if(operation=="note")editor.Note();else editor.Spacing();frames.Add(new{Operation=operation,Note=Preferences.Instance.NoteEntryMode.ToString(),Spacing=Preferences.Instance.PreferencesScroll.SpacingMode.ToString()});}output.Add(new{Frames=frames});}Console.Write(JsonSerializer.Serialize(output));}}
'''
with tempfile.TemporaryDirectory(prefix='deadsync-mode-original-') as temp:
 root=Path(temp);(root/'Program.cs').write_text(host);(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
 result=subprocess.run([a.dotnet,'run','--project',str(root/'Oracle.csproj'),'--verbosity','quiet'],check=True,capture_output=True,text=True)
 frames=json.loads(next(line for line in result.stdout.splitlines() if line.startswith('[')))
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(frames,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'SourceCommit':PIN,'EditorSha256':hashlib.sha256(source.encode()).hexdigest(),'Declarations':[hashlib.sha256(s.encode()).hexdigest() for s in parts],'HostSha256':hashlib.sha256(host.encode()).hexdigest(),'Scope':'Complete original two callbacks and two enum declarations; Preferences and Logger host stubs only. Original callbacks do not invoke an undo queue or lane/transport APIs. Not a whole Editor runtime execution.'},indent=2)+'\n')
print(f'Executed {len(frames)} original mode traces / {sum(len(t["Frames"]) for t in frames)} frames.')
