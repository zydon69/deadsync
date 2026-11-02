#!/usr/bin/env python3
"""Execute unchanged source notification guards and extension rule, without network/UI."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for path,rev in [(a.editor_root,pin),(a.reference_root/'Fumen',fp)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==rev
sources={}
def method(root,commit,path,signature):
 source=subprocess.check_output(['git','-C',str(root),'show',commit+':'+path],text=True,encoding='utf-8-sig');sources[path]=source
 start=source.index(signature);brace=source.index('{',start);end=brace+1;depth=1
 while depth:
  if source[end]=='{':depth+=1
  elif source[end]=='}':depth-=1
  end+=1
 return source[start:end]
update=method(a.editor_root,pin,'StepManiaEditor/Editor.cs','private void CheckForShowingUpdateModal()')
pack=method(a.editor_root,pin,'StepManiaEditor/EditorItgManiaPack.cs','private void CheckForShowingSongFileChangedNotification()')
opus=method(a.reference_root/'Fumen',fp,'Fumen/Sound/SoundManager.cs','public static bool HasOpusExtension(string path)')
adapter=r'''using System.Text.Json;using DateTime=Clock;
struct Clock{public double Seconds;public static Clock Now=>new(){Seconds=100};public static TimeSpan operator-(Clock a,Clock b)=>TimeSpan.FromSeconds(a.Seconds-b.Seconds);}
class Options{public bool SuppressUpdateNotification;}
class Preferences{public static Preferences Instance=new();public Options PreferencesOptions=new();}
static class UIFTUE{public static bool Completed;public static bool HasCompletedFTUE()=>Completed;}
static class ImGui{public static void TextLinkOpenURL(string url){}public static void Checkbox(string label,ref bool value){}}
static class UIModals{public static int Count;public static void OpenModalOneButton(string a,string b,string c,Action d,Action e){Count++;}}
class Editor{public bool HasShownUpdateModal;public Version Latest,Current;public string Url;public Version GetAppLatestVersion()=>Latest;public Version GetAppVersion()=>Current;public string GetLatestVersionUrl()=>Url;public string GetPrettyVersion(Version v)=>v.ToString();public void Run()=>CheckForShowingUpdateModal();UPDATE}
class Pack{public bool Saving;public Clock LastSaveCompleteTime;public int Count;public void ShowFileChangedModal()=>Count++;public void Run()=>CheckForShowingSongFileChangedNotification();PACK}
class SoundManager{OPUS}
class Program{static void Main(){var updates=new List<object>();foreach(var ftue in new[]{false,true})foreach(var suppress in new[]{false,true})foreach(var latest in new[]{"0.9","1.0","2.0"})foreach(var url in new[]{"","https://example.test/release"}){UIFTUE.Completed=ftue;Preferences.Instance.PreferencesOptions.SuppressUpdateNotification=suppress;UIModals.Count=0;var editor=new Editor{Latest=Version.Parse(latest),Current=Version.Parse("1.0"),Url=url};editor.Run();var first=UIModals.Count;editor.Latest=Version.Parse("9.0");editor.Run();updates.Add(new{Ftue=ftue,Suppress=suppress,Latest=latest,Url=url,First=first,Second=UIModals.Count});}var packs=new List<object>();foreach(var saving in new[]{false,true})foreach(var seconds in new[]{0.0,2.999,3.0,4.0}){var pack=new Pack{Saving=saving,LastSaveCompleteTime=new Clock{Seconds=100-seconds}};pack.Run();packs.Add(new{Saving=saving,Seconds=seconds,Shown=pack.Count});}var extensions=new List<object>();foreach(var path in new string[]{null,"","x.opus","x.OPUS","x.ogg","x.oga","dir.opus/x.wav","x.opus.backup"})extensions.Add(new{Path=path,Opus=SoundManager.HasOpusExtension(path)});Console.Write(JsonSerializer.Serialize(new{Updates=updates,Packs=packs,Extensions=extensions}));}}
'''.replace('UPDATE',update).replace('PACK',pack).replace('class SoundManager{OPUS}','class SoundManager{'+opus+'}')
with tempfile.TemporaryDirectory(prefix='notifications-source-')as d:
 t=Path(d);(t/'Program.cs').write_text(adapter);(t/'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup></Project>');subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);dll=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(dll)],text=True));assembly=hashlib.sha256(dll.read_bytes()).hexdigest()
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fp,'SourceHashes':{key:hashlib.sha256(value.encode()).hexdigest()for key,value in sources.items()},'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':assembly,'Scope':'Unchanged original update guard, pack saving/three-second guard and extension rule. Version getters, modal rendering and deterministic DateTime are explicit stand-ins. No network/OS watcher/decoder behavior is certified.'},indent=2)+'\n');print('Executed',len(result['Updates'])+len(result['Packs'])+len(result['Extensions']),'source notification cases.')
