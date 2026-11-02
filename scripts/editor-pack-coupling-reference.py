#!/usr/bin/env python3
"""Execute original pack/song unsaved modal and Close/Exit dispatch with UI-only adapters."""
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
methods='\n'.join(member(s)for s in ['private void ShowUnsavedChangesModal(','private void TryInvokePostSaveFunction()','private void OnClose()','private void OnExit()'])
cases=[dict(name=f'{intent}-{song}-{pack}-{choice}',Intent=intent,Song=song,Pack=pack,Choice=choice)for intent in ['close','exit','open','same-pack-open','pack-reload']for song in [False,True]for pack in [False,True]for choice in ['cancel','discard','save']]
program=r'''
using System;using System.Collections.Generic;using System.Text.Json;
public class Input{public string name{get;set;}="";public string Intent{get;set;}="";public string Choice{get;set;}="";public bool Song{get;set;}public bool Pack{get;set;}}
static class MainThreadDispatcher{public static void RunOnMainThread(Action action)=>action();}
static class ItgManiaPack{public const string FileName="Pack.ini";}
static class EditorPack{public static bool AreSongsInSamePack(string a,string b)=>b=="same";}
class Song{public string Title="Song";public string GetFileFullPath()=>"current";}
static class UIModals{public static string Title="";public static Action Cancel=()=>{};public static Action Discard=()=>{};public static Action Save=()=>{};public static void OpenModalThreeButtons(string title,string message,string c,Action cancel,string d,Action discard,string s,Action save){Title=title;Cancel=cancel;Discard=discard;Save=save;}}
class Pack{List<string> trace;public Pack(List<string>t){trace=t;}public void SaveItgManiaPack(bool fromSong,Action complete){trace.Add("pack-save");complete();}}
class Editor{Input c;Song ActiveSong=new();Pack ActivePack;Action? PostSaveFunction;List<string>trace=new();public Editor(Input value){c=value;ActivePack=new(trace);}bool HasUnsavedSongChanges()=>c.Song;bool HasUnsavedPackChanges()=>c.Pack;bool HasUnsavedSongOrPackChanges()=>c.Song||c.Pack;bool SaveLoadCloseEarlyOut()=>false;bool IsSaving()=>false;bool CanSaveWithoutLocationPrompt()=>true;void OnSaveAs()=>OnSave();void OnSave(){trace.Add("song-save");ActivePack.SaveItgManiaPack(true,TryInvokePostSaveFunction);}void OnCloseNoSave()=>trace.Add("close");void OnExitNoSave()=>trace.Add("exit");
''' +methods+r'''
public object Run(){switch(c.Intent){case "close":OnClose();break;case "exit":OnExit();break;default:PostSaveFunction=()=>trace.Add(c.Intent);ShowUnsavedChangesModal(c.Intent=="pack-reload",c.Intent=="same-pack-open"?"same":c.Intent=="open"?"different":null);break;}switch(c.Choice){case "cancel":UIModals.Cancel();break;case "discard":UIModals.Discard();break;case "save":UIModals.Save();break;}return new{c.name,title=UIModals.Title,trace,continuation_pending=PostSaveFunction!=null};}}
class Program{public static void Main(){var cases=JsonSerializer.Deserialize<List<Input>>(Console.In.ReadToEnd())!;var results=new List<object>();foreach(var c in cases){UIModals.Title="";UIModals.Cancel=()=>{};UIModals.Discard=()=>{};UIModals.Save=()=>{};results.Add(new Editor(c).Run());}Console.Write(JsonSerializer.Serialize(results));}}
'''
a.output.mkdir(parents=True,exist_ok=True);request=a.output/'request.json';request.write_text(json.dumps(cases,indent=2)+'\n')
with tempfile.TemporaryDirectory(prefix='deadsync-pack-oracle-')as folder:
 root=Path(folder);(root/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net7.0</TargetFramework><Nullable>enable</Nullable></PropertyGroup></Project>');(root/'Program.cs').write_text(program)
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'--nologo','-v','quiet'],check=True)
 assembly=root/'bin/Debug/net7.0/Oracle.dll';result=subprocess.run([str(a.dotnet),str(assembly)],input=json.dumps(cases),text=True,capture_output=True,check=True);expected=a.output/'expected.json';expected.write_text(json.dumps(json.loads(result.stdout),indent=2)+'\n');assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
sha=lambda value:hashlib.sha256(value).hexdigest()
provenance=dict(editor_commit=PIN,source_blob_sha256=sha(blob),extracted_methods_sha256=sha(methods.encode()),program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Unchanged ShowUnsavedChangesModal, TryInvokePostSaveFunction, OnClose, OnExit. UI delegate/property-only adapters. Save stubs record source documented song-to-pack callback coupling, not actual EditorSong writer/pack IO; no failure/durability claim.',request_sha256=sha(request.read_bytes()),expected_sha256=sha(expected.read_bytes()))
(a.output/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n');print(json.dumps(dict(cases=len(cases),output=str(a.output))))
