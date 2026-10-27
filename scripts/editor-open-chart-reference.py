#!/usr/bin/env python3
"""Run unchanged pinned SelectBestChart/DefaultChartListProvider and real Fumen preference enum converter."""
import argparse,hashlib,json,subprocess,tempfile,re
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2';LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
def blob(root,pin,path):return subprocess.check_output(['git','-C',str(root),'show',f'{pin}:{path}'])
for root,pin in [(a.reference_root,LIB),(a.reference_root/'Fumen',FUMEN)]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
paths={'song':(a.editor_root,PIN,'StepManiaEditor/EditorSong.cs'),'chart':(a.editor_root,PIN,'StepManiaEditor/EditorChart.cs'),'editor':(a.editor_root,PIN,'StepManiaEditor/Editor.cs'),'provider':(a.editor_root,PIN,'StepManiaEditor/DefaultChartListProvider.cs'),'interface':(a.editor_root,PIN,'StepManiaEditor/IActiveChartListProvider.cs'),'options':(a.editor_root,PIN,'StepManiaEditor/Preferences/PreferencesOptions.cs'),'converter':(a.reference_root/'Fumen',FUMEN,'Fumen/PermissiveEnumJsonConverterFactory.cs')}
b={k:blob(*v) for k,v in paths.items()};text={k:v.decode('utf-8-sig') for k,v in b.items()}
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);depth=0
 for end in range(brace,len(text)):
  if text[end]=='{':depth+=1
  elif text[end]=='}':depth-=1
  if depth==0:return text[start:end+1]
 raise ValueError(signature)
select=member(text['song'],'public EditorChart SelectBestChart(')
comparer=text['chart'][text['chart'].index('internal sealed class ChartComparer :'):]
start=text['editor'].index('public static readonly ChartType[] SupportedChartTypes =');supported=text['editor'][start:text['editor'].index(';',start)+1]
constants='\n'.join(re.findall(r'public const (?:ChartType|ChartDifficultyType) DefaultDefault(?:Steps|Difficulty)Type = .*?;',text['options']))
cases=[]
def chart(kind='dance-single',difficulty=4,rating=9,name='same',description='same',step_count=1,guid=None):
 return dict(steps_type=kind,difficulty=difficulty,rating=rating,name=name,description=description,step_count=step_count,guid=guid or f'00000000-0000-0000-0000-{len(cases):012x}')
def add(name,charts,options=None):cases.append(dict(Name=name,Charts=charts,Options=options or {}))
add('empty',[])
add('preferred-exact',[chart(difficulty=3,rating=99),chart(difficulty=4,rating=1),chart('dance-double',4,100)])
add('preferred-difficulty-before-meter',[chart(difficulty=5,rating=99),chart(difficulty=0,rating=1),chart(difficulty=3,rating=1)])
for kind,pair in [('dance_single','dance-double'),('dance_double','dance-single'),('pump_single','pump-double'),('pump_double','pump-single')]:
 add('paired-'+kind,[chart('smx-single',4),chart(pair,5,99),chart(pair,1,1),chart(pair,3,2)],{'DefaultStepsType':kind,'DefaultDifficultyType':'Challenge'})
add('no-pair-fallback-type-order',[chart('smx-single',4),chart('pump-routine',4),chart('dance-double',0),chart('dance-routine',4)],{'DefaultStepsType':'dance_solo'})
add('fallback-does-not-seek-preferred-difficulty',[chart('smx-single',4),chart('dance-double',1),chart('dance-double',3)],{'DefaultStepsType':'smx_team','DefaultDifficultyType':'Hard'})
add('preferred-unknown-difficulty',[chart(difficulty=77),chart(difficulty=-1)],{'DefaultDifficultyType':'77'})
add('only-unknown-difficulties',[chart(difficulty=77),chart(difficulty=-1)])
add('unsupported-excluded',[chart('kb7-single',4),chart('dance-couple',4),chart('pump-couple',4)])
add('same-difficulty-lowest-meter',[chart(rating=15),chart(rating=9),chart(rating=11)])
add('same-meter-cultural-names',[chart(name='zebra'),chart(name='éclair'),chart(name='Ångström'),chart(name='')])
add('same-name-description-steps-guid',[chart(description='b'),chart(description='a',step_count=3),chart(description='a',step_count=2,guid='00000000-0000-0000-0000-000000000009'),chart(description='a',step_count=2,guid='00000000-0000-0000-0000-000000000001')])
for name,options in [
 ('missing',{}),('names',{'DefaultStepsType':'pump_double','DefaultDifficultyType':'Easy'}),('case-space',{'DefaultStepsType':'  PUMP_DOUBLE  ','DefaultDifficultyType':' hArD '}),('invalid-text',{'DefaultStepsType':'bad','DefaultDifficultyType':'bad'}),('hyphen',{'DefaultStepsType':'pump-double','DefaultDifficultyType':'hard'}),('numeric-string',{'DefaultStepsType':'8','DefaultDifficultyType':'3'}),('numeric-unknown',{'DefaultStepsType':'999','DefaultDifficultyType':'77'}),('negative-string',{'DefaultStepsType':'-1','DefaultDifficultyType':'-1'}),('json-number',{'DefaultStepsType':8,'DefaultDifficultyType':3}),('null',{'DefaultStepsType':None,'DefaultDifficultyType':None}),('boolean',{'DefaultStepsType':True,'DefaultDifficultyType':False}),('enum-combination',{'DefaultStepsType':'dance_double,pump_single','DefaultDifficultyType':'Easy,Hard'}),('mixed-numeric-combination',{'DefaultStepsType':'dance_single,8','DefaultDifficultyType':'Easy,3'}),('overflow',{'DefaultStepsType':'2147483648','DefaultDifficultyType':'2147483648'}),('nested-object',{'DefaultStepsType':{}}),('nested-array',{'DefaultDifficultyType':[]})]:
 add('prefs-'+name,[chart('dance-single',0),chart('dance-single',4),chart('pump-double',1),chart('pump-double',3)],options)
request={'Cases':cases,'Cultures':['en-US','fr-FR']}
program='''using System;using System.Linq;using System.Collections.Generic;using System.Globalization;using System.Text.Json;using Fumen;using static Fumen.Converters.SMCommon;using StepManiaEditor;
namespace StepManiaEditor {
sealed class Editor { SUPPORTED }
sealed class Options { CONSTANTS public ChartType DefaultStepsType=DefaultDefaultStepsType;public ChartDifficultyType DefaultDifficultyType=DefaultDefaultDifficultyType; }
sealed class EditorChart {public int Source;public ChartType ChartType;public ChartDifficultyType ChartDifficultyType;public int Rating;public string Name;public string Description;public int Steps;public Guid Id;public EditorChart GetStepTotals()=>this;public int GetStepCount()=>Steps;public Guid GetGuid()=>Id;}
sealed class EditorSong {
 readonly Dictionary<ChartType,List<EditorChart>> Charts=new();
 public EditorSong(JsonElement source){int index=0;foreach(var e in source.EnumerateArray()){var kind=Enum.Parse<ChartType>(e.GetProperty("steps_type").GetString().Replace('-','_'));var c=new EditorChart{Source=index++,ChartType=kind,ChartDifficultyType=(ChartDifficultyType)e.GetProperty("difficulty").GetInt32(),Rating=e.GetProperty("rating").GetInt32(),Name=e.GetProperty("name").GetString(),Description=e.GetProperty("description").GetString(),Steps=e.GetProperty("step_count").GetInt32(),Id=e.GetProperty("guid").GetGuid()};if(!Editor.SupportedChartTypes.Contains(kind))continue;if(!Charts.ContainsKey(kind))Charts[kind]=new();Charts[kind].Add(c);}foreach(var list in Charts.Values)list.Sort(new ChartComparer());}
 public IReadOnlyList<EditorChart> GetCharts(ChartType type)=>Charts.GetValueOrDefault(type);
 public int[] Order()=>Editor.SupportedChartTypes.Where(Charts.ContainsKey).SelectMany(type=>Charts[type]).Select(c=>c.Source).ToArray();
 SELECT
}
COMPARER
}
class Program {static void Main(){var input=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;var options=new JsonSerializerOptions{IncludeFields=true};options.Converters.Add(new PermissiveEnumJsonConverterFactory());var results=input.GetProperty("Cultures").EnumerateArray().Select(culture=>{CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture.GetString());var cases=input.GetProperty("Cases").EnumerateArray().Select(item=>{try {var p=JsonSerializer.Deserialize<Options>(item.GetProperty("Options").GetRawText(),options);var song=new EditorSong(item.GetProperty("Charts"));var provider=new DefaultChartListProvider(p.DefaultStepsType,p.DefaultDifficultyType);return (object)new {Name=item.GetProperty("Name").GetString(),Steps=(int)p.DefaultStepsType,Difficulty=(int)p.DefaultDifficultyType,Order=song.Order(),Selected=provider.GetChartToUseForFocusedChart(song)?.Source,Error=(string)null};}catch(Exception e){return (object)new{Name=item.GetProperty("Name").GetString(),Error=e.GetType().Name};}}).ToArray();return new{Culture=culture.GetString(),Cases=cases};}).ToArray();Console.Write(JsonSerializer.Serialize(new{Results=results}));}}
'''.replace('SUPPORTED',supported).replace('CONSTANTS',constants).replace('SELECT',select).replace('COMPARER',comparer)
subprocess.run([str(a.dotnet),'build',str(a.reference_root/'Fumen/Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
with tempfile.TemporaryDirectory(prefix='deadsync-open-chart-') as folder:
 root=Path(folder);(root/'Program.cs').write_text(program);(root/'Provider.cs').write_text(text['provider']);(root/'Interface.cs').write_text(text['interface'])
 (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{a.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
 assembly=root/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly)],input=json.dumps(request),capture_output=True,text=True,check=True);result=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True)
request_path=a.output.with_suffix('.request.json');expected_path=a.output.with_suffix('.expected.json');request_path.write_text(json.dumps(request,indent=2,ensure_ascii=False)+'\n');expected_path.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
sha=lambda b:hashlib.sha256(b).hexdigest()
provenance=dict(editor_commit=PIN,library_commit=LIB,fumen_commit=FUMEN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Unchanged SelectBestChart, DefaultChartListProvider, ChartComparer, supported table and default constants; property-only EditorSong/EditorChart adapters. Actual Fumen enum JSON converter. Sorting is real CurrentCulture. Not full EditorSong loading or native lifecycle.',source_blobs={k:dict(path=paths[k][2],git_blob=subprocess.check_output(['git','-C',str(paths[k][0]),'rev-parse',f'{paths[k][1]}:{paths[k][2]}'],text=True).strip(),sha256=sha(v))for k,v in b.items()},extracted_select_sha256=sha(select.encode()),program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,request_sha256=sha(request_path.read_bytes()),expected_sha256=sha(expected_path.read_bytes()))
a.output.with_suffix('.reference.json').write_text(json.dumps(provenance,indent=2)+'\n')
print(f'Wrote {len(cases)} cases in two cultures')
