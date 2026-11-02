#!/usr/bin/env python3
"""Execute pinned original File-change callbacks with property/UI-only adapters."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
blob=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{PIN}:StepManiaEditor/Editor.cs']);source=blob.decode('utf-8-sig')
def member(signature):
 start=source.index(signature);brace=source.index('{',start);depth=0
 for end in range(brace,len(source)):
  if source[end]=='{':depth+=1
  elif source[end]=='}':depth-=1
  if depth==0:return source[start:end+1]
 raise ValueError(signature)
methods='\n'.join(member(s)for s in ['private void ShowFileChangedModal()','private void OnSongFileChangedNotification(object sender, FileSystemEventArgs e)','private void CheckForShowingSongFileChangedNotification()'])
cases=[dict(name='normal',Seconds=4),dict(name='no_song',NoSong=True,Seconds=4),dict(name='saving',Saving=True,Seconds=4),dict(name='grace',Seconds=1),dict(name='already_showing',Showing=True,Seconds=4),dict(name='suppressed',Suppressed=True,Seconds=4),dict(name='ignore',Choice='ignore',Seconds=4),dict(name='reload_dirty_song',Choice='reload',DirtySong=True,Seconds=4),dict(name='reload_dirty_both',Choice='reload',DirtySong=True,DirtyPack=True,Seconds=4),dict(name='suppress_checkbox',Toggle=True,Seconds=4),dict(name='created_event',Event='Created',Seconds=4)]
program=r'''
using System;using System.IO;using System.Collections.Generic;using System.Text.Json;
public class Input{public string name{get;set;}="";public double Seconds{get;set;}=4;public bool NoSong{get;set;}public bool Saving{get;set;}public bool Showing{get;set;}public bool Suppressed{get;set;}public bool DirtySong{get;set;}public bool DirtyPack{get;set;}public bool Toggle{get;set;}public string Choice{get;set;}="";public string Event{get;set;}="Changed";}
class Options{public bool SuppressExternalSongModificationNotification;}
class Preferences{public static Preferences Instance=new();public Options PreferencesOptions=new();}
static class ItgManiaPack{public const string FileName="Pack.ini";}
enum ImGuiCol{Text}enum LogLevel{Warn}
static class UILog{public static int GetColor(LogLevel level)=>0;}
static class ImGui{public static bool Toggle;public static List<string> Warnings=new();public static void PushStyleColor(ImGuiCol col,int color){}public static void PopStyleColor(){}public static void Separator(){}public static void TextUnformatted(string value){Warnings.Add(value);}public static void Checkbox(string label,ref bool value){if(Toggle)value=true;}}
static class MainThreadDispatcher{public static void RunOnMainThread(Action action)=>action();}
static class UIModals{public static int Count;public static Action Ignore=()=>{};public static Action Reload=()=>{};public static void OpenModalTwoButtons(string title,string message,string left,Action ignore,string right,Action reload,Action content){Count++;Ignore=ignore;Reload=reload;content();}}
class Song{Input c;public Song(Input value){c=value;}public string GetFileName()=>"song.ssc";public bool IsSaving()=>c.Saving;public DateTime GetLastSaveCompleteTime()=>DateTime.Now.AddSeconds(-c.Seconds);}
class Editor{Input c;Song? ActiveSong;bool ShowingSongFileChangedNotification;int Reloads;bool Forced;public Editor(Input value){c=value;ActiveSong=c.NoSong?null:new Song(c);ShowingSongFileChangedNotification=c.Showing;}bool HasUnsavedSongChanges()=>c.DirtySong;bool HasUnsavedPackChanges()=>c.DirtyPack;void OnReload(bool force){Reloads++;Forced=force;}
''' + methods + r'''
public object Run(){OnSongFileChangedNotification(this,new FileSystemEventArgs(Enum.Parse<WatcherChangeTypes>(c.Event),"/source","song.ssc"));if(c.Choice=="ignore")UIModals.Ignore();if(c.Choice=="reload")UIModals.Reload();return new {c.name,shown=ShowingSongFileChangedNotification,modal_count=UIModals.Count,reload_count=Reloads,reload_forced=Forced,suppressed=Preferences.Instance.PreferencesOptions.SuppressExternalSongModificationNotification,warnings=ImGui.Warnings};}}
class Program{public static void Main(){var cases=JsonSerializer.Deserialize<List<Input>>(Console.In.ReadToEnd())!;var results=new List<object>();foreach(var c in cases){Preferences.Instance=new();Preferences.Instance.PreferencesOptions.SuppressExternalSongModificationNotification=c.Suppressed;UIModals.Count=0;UIModals.Ignore=()=>{};UIModals.Reload=()=>{};ImGui.Toggle=c.Toggle;ImGui.Warnings=new();results.Add(new Editor(c).Run());}Console.Write(JsonSerializer.Serialize(results));}}
'''
a.output.mkdir(parents=True,exist_ok=True);request=a.output/'request.json';request.write_text(json.dumps(cases,indent=2)+'\n')
with tempfile.TemporaryDirectory(prefix='deadsync-external-oracle-')as folder:
 root=Path(folder);(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><Nullable>enable</Nullable></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'--nologo','-v','quiet'],check=True)
 assembly=root/'bin/Debug/net10.0/Oracle.dll';result=subprocess.run([str(a.dotnet),str(assembly)],input=json.dumps(cases),text=True,capture_output=True,check=True);expected=a.output/'expected.json';expected.write_text(json.dumps(json.loads(result.stdout),indent=2)+'\n');assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
sha=lambda value:hashlib.sha256(value).hexdigest()
provenance=dict(editor_commit=PIN,source_path='StepManiaEditor/Editor.cs',source_blob_sha256=sha(blob),extracted_methods_sha256=sha(methods.encode()),program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Unchanged complete ShowFileChangedModal, OnSongFileChangedNotification and CheckForShowingSongFileChangedNotification. Property-only song, preferences, immediate main-thread dispatcher and captured UI delegates. No real filesystem watcher, full EditorSong reload, rendering or native IO claim.',request_sha256=sha(request.read_bytes()),expected_sha256=sha(expected.read_bytes()))
(a.output/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n');print(json.dumps(dict(cases=len(cases),output=str(a.output))))
