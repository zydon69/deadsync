#!/usr/bin/env python3
"""Execute original auto-assets callback/action and real Fumen Undo/Redo."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser()
for name in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();GA='48e18f71655d0f0d86144c63239331833501fda2';FM='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for directory,pin in [(a.editor_root,GA),(a.reference_root/'Fumen',FM)]:assert subprocess.check_output(['git','-C',str(directory),'rev-parse','HEAD'],text=True).strip()==pin
sources={}
def source(name):
 raw=subprocess.check_output(['git','-C',str(a.editor_root),'show',GA+':StepManiaEditor/'+name]);sources[name]=hashlib.sha256(raw).hexdigest();return raw.decode('utf-8-sig')
def member(text,sig):
 i=text.index(sig);j=text.index('{',i)+1;depth=1
 while depth:
  depth+=(text[j]=='{')-(text[j]=='}');j+=1
 return text[i:j]
editor=source('Editor.cs');utils=source('Utils.cs');images=source('EditorSongImageUtils.cs');action=source('EditorActions/ActionSetSongAssets.cs')
reflection='\n'.join(member(utils,sig) for sig in ['public static void SetFieldOrPropertyToValue<T>','public static T GetValueFromFieldOrProperty<T>','public static bool IsField'])
callback=member(editor,'private void ApplyBestSongAssets(bool ifUnset)').replace('private void','public void',1)
code='''using System.Reflection;using Fumen;namespace StepManiaEditor {
internal static class Utils {public static readonly string[] ExpectedImageFormats=["bmp","gif","jpeg","jpg","png","tif","tiff","webp"];public static readonly string[] ExpectedVideoFormats=["avi","f4v","flv","mkv","mp4","mpeg","mpg","mov","ogv","webm","wmv"];public static readonly string[] ExpectedLyricsFormats=["lrc"]; REFLECTION }
internal static class ImGuiUtils {public const int BackgroundWidthDefaultDPI=640,BackgroundHeightDefaultDPI=480,BannerWidthDefaultDPI=418,BannerHeightDefaultDPI=164;}
internal class EditorSong {public string Directory;public string BackgroundPath{get;set;}public string BannerPath{get;set;}public string CDTitlePath{get;set;}public string JacketPath{get;set;}public string CDImagePath{get;set;}public string DiscImagePath{get;set;}public string LyricsPath{get;set;}public string GetFileDirectory()=>Directory;}
internal static class Logger {public static List<string> Messages=[];public static void Info(string message)=>Messages.Add(message);}
internal class Editor {public EditorSong ActiveSong;public bool Editable=true;public bool CanEdit()=>Editable; CALLBACK }
}
namespace SkiaSharp {internal sealed class SKCodec:IDisposable {public (int Width,int Height) Info=>(1,1);public static SKCodec Create(string path)=>new();public void Dispose(){}}}
'''.replace('REFLECTION',reflection).replace('CALLBACK',callback)
program='''using StepManiaEditor;using Fumen;using System.Text.Json;using Logger=StepManiaEditor.Logger;
class Program {
static readonly string[] Properties=["BackgroundPath","BannerPath","CDTitlePath","JacketPath","CDImagePath","DiscImagePath","LyricsPath"];
static readonly string[] Files=["background.png","banner.png","cdtitle.png","jacket.png","cover-cd.png","cover disc.png","words.lrc"];
static object Snapshot(EditorSong song)=>Properties.ToDictionary(p=>p,p=>(string)song.GetType().GetProperty(p).GetValue(song));
static void Main(){var output=new List<object>();var root=System.IO.Path.Combine(System.IO.Path.GetTempPath(),"deadsync-auto-assets-"+Guid.NewGuid());System.IO.Directory.CreateDirectory(root);try{foreach(var mode in new[]{"all","unset","same","empty","missing","blocked","chartless","links"}){ActionQueue.Instance.Clear();Logger.Messages.Clear();var directory=System.IO.Path.Combine(root,mode);if(mode!="missing")System.IO.Directory.CreateDirectory(directory);if(mode!="missing"&&mode!="empty"&&mode!="links")foreach(var name in Files)System.IO.File.WriteAllBytes(System.IO.Path.Combine(directory,name),[]);if(mode=="links"){var targetDir=System.IO.Path.Combine(root,"linked-directory");System.IO.Directory.CreateDirectory(targetDir);var targetFile=System.IO.Path.Combine(root,"linked-file.dat");System.IO.File.WriteAllBytes(targetFile,[]);System.IO.File.CreateSymbolicLink(System.IO.Path.Combine(directory,"background.png"),System.IO.Path.Combine(root,"missing-image.dat"));System.IO.File.CreateSymbolicLink(System.IO.Path.Combine(directory,"cdtitle.png"),targetFile);System.IO.Directory.CreateSymbolicLink(System.IO.Path.Combine(directory,"banner.png"),targetDir);}var inventory=mode=="missing"?Array.Empty<string>():System.IO.Directory.GetFiles(directory).Select(System.IO.Path.GetFileName).ToArray();var song=new EditorSong{Directory=directory};for(var i=0;i<Properties.Length;i++){string value=mode=="same"?Files[i]:mode=="unset"?(i%2==0?"old-"+i: i==3?" ":""):"old-"+i;typeof(EditorSong).GetProperty(Properties[i]).SetValue(song,value);}var e=new Editor{ActiveSong=song,Editable=mode!="blocked"};var before=Snapshot(song);e.ApplyBestSongAssets(mode=="unset");var after=Snapshot(song);var undo=ActionQueue.Instance.Undo()?.GetType().Name;var afterUndo=Snapshot(song);var secondUndo=ActionQueue.Instance.Undo()?.GetType().Name;ActionQueue.Instance.Redo();output.Add(new{Name=mode,OnlyUnset=mode=="unset",CanEdit=e.Editable,Inventory=inventory,Before=before,After=after,Undo=undo,AfterUndo=afterUndo,SecondUndo=secondUndo,AfterRedo=Snapshot(song),Messages=Logger.Messages.ToArray()});}}finally{System.IO.Directory.Delete(root,true);}Console.Write(JsonSerializer.Serialize(output));}}
'''
with tempfile.TemporaryDirectory(prefix='auto-assets-source-') as folder:
 root=Path(folder)
 for name,content in [('Adapter.cs',code),('Program.cs',program),('Images.cs',images),('Action.cs',action)]: (root/name).write_text(content)
 library=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{library}</HintPath></Reference></ItemGroup></Project>');subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=root/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(EditorCommit=GA,FumenCommit=FM,SourcesSha256=sources,AdapterSha256=hashlib.sha256((code+program).encode()).hexdigest(),AssemblySha256=sha,FumenAssemblySha256=hashlib.sha256(library.read_bytes()).hexdigest(),Scope='Original callback, complete ActionSetSongAssets, whole EditorSongImageUtils and three original reflection helpers; real pinned Fumen ActionQueue/UndoableAction, real temporary filesystem enumeration. Host supplies song string properties and CanEdit; Skia codec dimensions fixed1x1, so real image decoding is not proven (named candidates are used). Chartless is the same song callback with no chart dependency.'),indent=2)+'\n');print('Executed',len(result),'original auto-assets action/history cases.')
