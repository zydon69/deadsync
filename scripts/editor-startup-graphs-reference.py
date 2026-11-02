#!/usr/bin/env python3
"""Execute unchanged source startup-graph selection block with real Fumen enums."""
from pathlib import Path
import argparse,subprocess,tempfile,hashlib,json,re
p=argparse.ArgumentParser()
for k in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+k,type=Path,required=True)
a=p.parse_args();pin='48e18f71655d0f0d86144c63239331833501fda2';fp='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for path,rev in [(a.editor_root,pin),(a.reference_root/'Fumen',fp)]:assert subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()==rev
source=subprocess.check_output(['git','-C',str(a.editor_root),'show',pin+':StepManiaEditor/Editor.cs'],text=True)
a0=source.index('public static readonly ChartType[] SupportedSinglePlayerChartTypes =');b=source.index('];',a0)+2;types=source[a0:b]
a0=source.index('var pOptions = Preferences.Instance.PreferencesOptions;',source.index('private async Task InitStepGraphDataAsync()'));b=source.index('if (validStepGraphTypes.Count > 0)',a0);block=source[a0:b]
adapter='''using System.Text.Json;using Fumen.Converters;using static Fumen.Converters.SMCommon;
class Prefs{public HashSet<ChartType> StartupStepGraphs=[];}
class Preferences{public static Preferences Instance=new();public Prefs PreferencesOptions=new();}
class Program{'''+types+'''
static List<ChartType> Select(HashSet<ChartType> selected,HashSet<ChartType> missing){Preferences.Instance.PreferencesOptions.StartupStepGraphs=selected;var PadDataByChartType=new Dictionary<ChartType,object>();foreach(var type in SupportedSinglePlayerChartTypes)PadDataByChartType[type]=missing.Contains(type)?null:new object();
'''+block+'''
return validStepGraphTypes;}
static void Main(){var cases=new List<object>();foreach(var selected in new[]{new int[]{0,1},new int[]{},new int[]{0,1,2,3,4,5,6,7,8,9,10,38,39,40,41,42,999},new int[]{3,38,41},new int[]{2,5,9,10,42,999}})foreach(var missing in new[]{new int[]{},new int[]{0,3,41}}){var result=Select(selected.Select(x=>(ChartType)x).ToHashSet(),missing.Select(x=>(ChartType)x).ToHashSet());cases.Add(new{Selected=selected,MissingPads=missing,Layouts=result.Select(x=>x.ToString().Replace('_','-')).ToArray()});}Console.Write(JsonSerializer.Serialize(cases));}}
'''
with tempfile.TemporaryDirectory(prefix='startup-graphs-oracle-')as d:
 t=Path(d);(t/'Program.cs').write_text(adapter);dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll';(t/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>');subprocess.run([str(a.dotnet),'build',str(t/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=t/'bin/Release/net10.0/Oracle.dll';result=json.loads(subprocess.check_output([str(a.dotnet),str(assembly)],text=True));sha=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':pin,'FumenCommit':fp,'SourceTypesSha256':hashlib.sha256(types.encode()).hexdigest(),'SourceSelectionSha256':hashlib.sha256(block.encode()).hexdigest(),'AdapterSha256':hashlib.sha256(adapter.encode()).hexdigest(),'AssemblySha256':sha,'Scope':'Unchanged original startup graph selection block and actual source supported-single-player types; real Fumen ChartType enum. Explicit pad availability stand-in. Does not claim to execute source async pad/graph decoding or root-node creation.'},indent=2)+'\n');print('Executed',len(result),'original startup graph selection cases.')
