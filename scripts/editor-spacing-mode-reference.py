#!/usr/bin/env python3
"""Whole original Scroll + real converter/Undo, original factory and toggle."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for path,rev in[(a.editor_root,pin),(a.reference_root/'Fumen',fp)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==rev
sources={}
def source(path):
 value=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':'+path],text=True,encoding='utf-8-sig');sources[path]=value;return value
def method(text,signature):
 start=text.index(signature);brace=text.index('{',start);end=brace+1;depth=1
 while depth:
  if text[end]=='{':depth+=1
  elif text[end]=='}':depth-=1
  end+=1
 return text[start:end]
preferences=source('StepManiaEditor/Preferences/PreferencesScroll.cs');editor=source('StepManiaEditor/Editor.cs');factory=method(source('StepManiaEditor/EventSpacingHelper.cs'),'public static EventSpacingHelper GetSpacingHelper(EditorChart activeChart)');toggle=method(editor,'private void OnToggleSpacingMode()');mini=method(editor,'private SpacingMode GetMiniMapSpacingMode()')
adapter=r'''using System.Text.Json;using Fumen;using Fumen.Converters;using StepManiaEditor;
namespace StepManiaEditor{
class Editor{public enum SpacingMode{ConstantTime,ConstantRow,Variable}public enum WaveFormScrollMode{CurrentTempo,CurrentTempoAndRate,MostCommonTempo}public void Toggle()=>OnToggleSpacingMode();public SpacingMode Mini()=>GetMiniMapSpacingMode();__TOGGLE__ __MINI__}
class Preferences{public static Preferences Instance=new();public PreferencesScroll PreferencesScroll=new();public MiniMapPrefs PreferencesMiniMap=new();}class MiniMapPrefs{public Editor.SpacingMode MiniMapSpacingModeForVariable=Editor.SpacingMode.ConstantRow;}
class EditorChart{}class EventSpacingHelper{__FACTORY__}class EventSpacingHelperConstantTime:EventSpacingHelper{public EventSpacingHelperConstantTime(EditorChart chart){}}class EventSpacingHelperConstantRow:EventSpacingHelper{public EventSpacingHelperConstantRow(EditorChart chart){}}class EventSpacingHelperVariable:EventSpacingHelper{public EventSpacingHelperVariable(EditorChart chart){}}static class Logger{public static void Info(string text){}}
}
class Program{static void Main(){var options=new JsonSerializerOptions{IncludeFields=true};options.Converters.Add(new PermissiveEnumJsonConverterFactory());var output=new List<object>();foreach(var token in new string[]{"null","true","0","1","2","99","\"ConstantTime\"","\"constantrow\"","\"Variable\"","\"1\"","\"2\"","\"99\"","\"-2\"","\"2147483647\"","\"future\"","\"ConstantRow, Variable\"","{}","[]"}){var input="{\"ShowScrollControlPreferencesWindow\":true,\"SpacingMode\":"+token+"}";try{var p=JsonSerializer.Deserialize<PreferencesScroll>(input,options);Preferences.Instance.PreferencesScroll=p;var raw=(int)p.SpacingMode;var serialized=JsonSerializer.SerializeToElement(p,options).GetProperty("SpacingMode");var helper=EventSpacingHelper.GetSpacingHelper(null).GetType().Name;var editor=new Editor();var minimap=(int)editor.Mini();var defaults=p.IsUsingDefaults();p.RestoreDefaults();var restored=(int)p.SpacingMode;var count=ActionQueue.Instance.GetUndoCount();if(!defaults)ActionQueue.Instance.Undo();var undone=(int)p.SpacingMode;if(!defaults)ActionQueue.Instance.Redo();var redone=(int)p.SpacingMode;p.SpacingMode=(Editor.SpacingMode)raw;editor.Toggle();var toggled=(int)p.SpacingMode;output.Add(new{Input=JsonDocument.Parse(token).RootElement.Clone(),Raw=raw,Serialized=serialized,Helper=helper,MiniMap=minimap,UsingDefaults=defaults,Restored=restored,Undone=undone,Redone=redone,Toggled=toggled,Window=p.ShowScrollControlPreferencesWindow,Error=false});ActionQueue.Instance.Clear();}catch(JsonException){output.Add(new{Input=JsonDocument.Parse(token).RootElement.Clone(),Error=true});}}Console.Write(JsonSerializer.Serialize(output));}}
'''.replace('__TOGGLE__',toggle).replace('__MINI__',mini).replace('__FACTORY__',factory).replace('var count=ActionQueue.Instance.GetUndoCount();','')
with tempfile.TemporaryDirectory(prefix='spacing-mode-source-')as d:
 t=Path(d);(t/'Preferences.cs').write_text(preferences);(t/'Program.cs').write_text(adapter);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(t/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>');subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fp,'SourceHashes':{key:hashlib.sha256(value.encode()).hexdigest()for key,value in sources.items()},'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':sha,'Scope':'Whole original PreferencesScroll and Restore/Undo/Redo, real Fumen permissive converter with no registered defaults for Scroll, original EventSpacingHelper.GetSpacingHelper, Editor.OnToggleSpacingMode and GetMiniMapSpacingMode methods. Helper subclasses are constructor/type-name stand-ins, not rendering.'},indent=2)+'\n');print('Executed',len(result),'original spacing converter/factory/toggle/Restore cases.')
