#!/usr/bin/env python3
"""Execute entire original Options preferences with real Fumen notifier and Undo queue."""
import argparse,subprocess,tempfile,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser()
for k in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+k,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for path,rev in [(a.editor_root,pin),(a.reference_root/'Fumen',fp)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==rev
source=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/Preferences/PreferencesOptions.cs'],text=True,encoding='utf-8-sig')
ui_source=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/ImGui/ImGuiUtils.cs'],text=True,encoding='utf-8-sig')
begin=ui_source.index('public static double GetDpiScale()');brace=ui_source.index('{',begin);depth=1;end=brace+1
while depth:
 if ui_source[end]=='{':depth+=1
 elif ui_source[end]=='}':depth-=1
 end+=1
method=ui_source[begin:end]
begin2=ui_source.index('public static int UiScaled(int value)');brace2=ui_source.index('{',begin2);depth=1;end2=brace2+1
while depth:
 if ui_source[end2]=='{':depth+=1
 elif ui_source[end2]=='}':depth-=1
 end2+=1
method+='\n'+ui_source[begin2:end2]
adapter='using System.Text.Json;using Fumen;using StepManiaEditor;\nnamespace StepManiaEditor {class Preferences {public static Preferences Instance=new();public PreferencesOptions PreferencesOptions=new();} class MonitorProbe {public double Scale;public double GetMonitorDpiScale()=>Scale;} class DpiProbe {public static double DpiScale;public static MonitorProbe Editor;METHOD}}\nclass Program {static void Main(){var output=new List<object>();foreach(var (custom,scale,monitor) in new(bool,double,double)[]{(false,1,2),(true,1.5,2),(true,0,2),(true,-1,2),(true,0,-1),(false,1,0),(true,1e-10,2),(true,8,2),(true,.25,2),(true,1.99999999,2)}){DpiProbe.DpiScale=0;DpiProbe.Editor=new MonitorProbe{Scale=monitor};Preferences.Instance.PreferencesOptions.UseCustomDpiScale=custom;Preferences.Instance.PreferencesOptions.DpiScale=scale;var first=DpiProbe.GetDpiScale();var scaledTwo=DpiProbe.UiScaled(2);var closeWidth=DpiProbe.UiScaled(18);var fontPixels=DpiProbe.UiScaled(15);Preferences.Instance.PreferencesOptions.UseCustomDpiScale=true;Preferences.Instance.PreferencesOptions.DpiScale=3;DpiProbe.Editor.Scale=4;var second=DpiProbe.GetDpiScale();output.Add(new{Custom=custom,Scale=scale,Monitor=monitor,First=first,ScaledTwo=scaledTwo,CloseWidth=closeWidth,FontPixels=fontPixels,AfterPreferenceAndMonitorChange=second});}Console.Write(JsonSerializer.Serialize(output));}}\n'
adapter=adapter.replace('METHOD',method)
with tempfile.TemporaryDirectory(prefix='audio-preferences-oracle-') as d:
 t=Path(d);(t/'Audio.cs').write_text(source);(t/'Program.cs').write_text(adapter);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(t/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>');subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fp,'SourceSha256':hashlib.sha256(source.encode()).hexdigest(),'MethodSha256':hashlib.sha256(method.encode()).hexdigest(),'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':sha,'FumenAssemblySha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'Scope':'Unchanged original ImGuiUtils.GetDpiScale with real Fumen DoubleEquals, whole original PreferencesOptions and explicit monitor getter stand-in. Native OS DPI querying and rendering are not simulated.'},indent=2)+'\n');print('Executed',len(result),'original DPI lifetime-cache cases.')
