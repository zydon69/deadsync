#!/usr/bin/env python3
"""Execute the original Shift menu routing; actions are recorded by explicit host callbacks."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==pin
raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{pin}:StepManiaEditor/UI/UIEditEvents.cs']);text=raw.decode('utf-8-sig');start=text.index('private void DrawShiftEventsMenuItems(');end=text.index('{',start)+1;depth=1
while depth:
 depth+=(text[end]=='{')-(text[end]=='}');end+=1
method=text[start:end]
commands=['Mirror','Flip','MirrorAndFlip','ShiftRight','ShiftRightAndWrap','ShiftLeft','ShiftLeftAndWrap','ShiftEarlier','ShiftLater']
code='''using System.Text.Json;
class EditorEvent(int id){public int Id=id;}
class Chart { public IEnumerable<EditorEvent> GetEvents()=>new[]{new EditorEvent(1),new EditorEvent(2),new EditorEvent(3)};public bool CanBeEdited()=>true;}
class Editor { public static int[] Passed;public static string Action;public Chart GetFocusedChart()=>new();public int GetShiftNotesRows()=>48; CALLBACKS }
class Preferences {public static Preferences Instance=new();public Binds PreferencesKeyBinds=new();}class Binds {COMMANDS}
static class UIControls {public static string GetCommandString(string key)=>key;}
static class SMCommon {public const int MaxValidDenominator=48,NumBeatsPerMeasure=4;}
static class ImGui {public static string Choose;public static bool MenuItem(string label,string command)=>label.Split(" (")[0]==Choose;}
class UIEditEvents {Editor Editor=new();void PushDisabled(){}void PopDisabled(){} METHOD public void Run(bool all){DrawShiftEventsMenuItems(all?null:new[]{new EditorEvent(2)});} }
class Program {static void Main(){var result=new List<object>();foreach(var all in new[]{false,true})foreach(var choice in new[]{"Mirror","Flip","Mirror and Flip","Shift Right","Shift Right and Wrap","Shift Left","Shift Left and Wrap","Shift Earlier","Shift Later"}){ImGui.Choose=choice;Editor.Passed=null;new UIEditEvents().Run(all);result.Add(new{All=all,Choice=choice,Editor.Action,Editor.Passed});}Console.Write(JsonSerializer.Serialize(result));}}
'''
callbacks=''
for name in ['Mirror','Flip','MirrorAndFlip']:callbacks+=f'public void On{name}Selection(){{Action="{name}";Passed=new[]{{2}};}}'
for name in ['Right','RightAndWrap','Left','LeftAndWrap','Earlier','Later']:callbacks+=f'public void OnShiftNotes{name}(IEnumerable<EditorEvent> events){{Action="{name}";Passed=events.Select(e=>e.Id).ToArray();}}'
code=code.replace('CALLBACKS',callbacks).replace('COMMANDS',''.join(f'public string {c}="{c}";' for c in commands)).replace('METHOD',method)
with tempfile.TemporaryDirectory(prefix='deadsync-source-shift-routing-') as folder:
 root=Path(folder);(root/'Program.cs').write_text(code);(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup></Project>');subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);result=json.loads(subprocess.check_output([str(a.dotnet),str(root/'bin/Release/net10.0/Oracle.dll')],text=True))
for case in result:assert case['Passed']==([1,2,3] if case['All'] and case['Choice'].startswith('Shift ') else [2]),case
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'UIEditEventsSha256':hashlib.sha256(raw).hexdigest(),'AdapterSha256':hashlib.sha256(code.encode()).hexdigest(),'Scope':'Unchanged original menu routing; explicit callback adapters record passed event identities, do not emulate actions. Mirror callbacks consume current selection2, shift callbacks receive event list.'},indent=2)+'\n');print(f'Executed {len(result)} source routing branches.')
