#!/usr/bin/env python3
"""Probe pinned Fumen cross-format writers/readers; temporary synthetic files only."""
import argparse,base64,hashlib,json,re,subprocess,tempfile
from pathlib import Path
LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.reference_root/'Fumen'
for folder,pin in [(a.reference_root,LIB),(root,PIN)]:
 assert subprocess.check_output(['git','-C',str(folder),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(folder),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
GA='48e18f71655d0f0d86144c63239331833501fda2'
assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==GA
ga_paths=['StepManiaEditor/Editor.cs','StepManiaEditor/EditorSong.cs','StepManiaEditor/EditorChart.cs','StepManiaEditor/Preferences/Preferences.cs']
ga_blobs={path:subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:{path}'])for path in ga_paths}
paths=['Fumen/Converters/SMCommon.cs','Fumen/Converters/SMPropertyParsers.cs','Fumen/Converters/SMReader.cs','Fumen/Converters/SSCReader.cs','Fumen/Converters/MSDFile.cs','Fumen/Converters/SMWriter.cs','Fumen/Converters/SSCWriter.cs','Fumen/Converters/SMWriterBase.cs'];blobs={path:subprocess.check_output(['git','-C',str(root),'show',f'{PIN}:{path}'])for path in paths}
cases=[]
def chart(layout,notes,timing='',extra=''):
 return f'#NOTEDATA:;#STEPSTYPE:{layout};#DESCRIPTION:Credit;#DIFFICULTY:Hard;#METER:9;{timing}{extra}#NOTES:{notes};'
def add(name,content,source='ssc',**flags):
 for target in ['sm','ssc']:
  cases.append(dict(Name=name+'-'+target,Source=source,Target=target,Bytes=base64.b64encode(content.encode()).decode(),ForceTiming=flags.get('force',False),StepF2=flags.get('step',False),Outfox=flags.get('outfox',True),Custom=flags.get('custom',True)))
header='#VERSION:0.83;#TITLE:Save;#ARTIST:Artist;#CREDIT:Author;#MUSIC:music.ogg;#BPMS:0=120;#OFFSET:0;#LASTSECONDHINT:12;'
for timing in ['', '#BPMS:0=150;#OFFSET:0.1;', '#WARPS:1=1;', '#STOPS:1=-0.5;', '#LABELS:1=Label;#FAKES:2=1;#COMBOS:1=2;#SPEEDS:1=2=0=0;#SCROLLS:1=2;', '#ATTACKS:TIME=0:LEN=1:MODS=drunk;']:
 name='timing-'+str(len(cases));content=header+chart('dance-single','1000')+chart('dance-double','00000001',timing)
 add(name,content);add(name+'-force',content,force=True)
for layout in ['pump-routine','pump-couple']:
 for notes in ['1000000000&0000000001','4000000000\n3000000000&0000000001','L000000000&0000000001','1000000000&M000000000','1000000000&0000000000&0000000000&0000000000&0000000001']:
  add(layout+'-'+str(len(cases)),header+chart(layout,notes),step=True)
for layout,columns in [('smx-dual',6),('smx-full',10),('smx-team',10)]:
 for outfox in [True,False]:add(layout+'-'+str(outfox),header+chart(layout,'1'+'0'*(columns-1)),outfox=outfox)
add('custom-omit',header+chart('dance-single','1000'),custom=False)
add('sm-promote','#TITLE:SM;#BPMS:0=120;#STOPS:1=-0.5;#NOTES:dance-single:Credit:Hard:9::1000;',source='sm')
request=dict(Cases=cases)
program='''using System;using System.Linq;using System.IO;using System.Text.Json;using System.Collections.Generic;using Fumen;using Fumen.Converters;using Fumen.ChartDefinition;using static Fumen.Converters.SMWriterBase;using Path=System.IO.Path;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var folder=Path.Combine(Path.GetTempPath(),"deadsync-save-reference-"+Guid.NewGuid());Directory.CreateDirectory(folder);var result=new List<object>();
try{foreach(var item in request.GetProperty("Cases").EnumerateArray()){string error=null;string bytes=null;object before=null,after=null;bool saved=false;try{var src=Path.Combine(folder,Guid.NewGuid()+"."+item.GetProperty("Source").GetString());File.WriteAllBytes(src,Convert.FromBase64String(item.GetProperty("Bytes").GetString()));var song=await Reader.CreateReader(src).LoadAsync(CancellationToken.None);before=Project(song);var dst=Path.Combine(folder,Guid.NewGuid()+"."+item.GetProperty("Target").GetString());var custom=new SMWriterCustomProperties();custom.CustomSongProperties.Add("ReferenceCustom","song");foreach(var c in song.Charts)custom.CustomChartProperties.Add(new Dictionary<string,string>{{"ReferenceCustom","chart"}});var config=new SMWriterBaseConfig{Song=song,FilePath=dst,FallbackChart=song.Charts.FirstOrDefault(),ForceOnlySongLevelTiming=item.GetProperty("ForceTiming").GetBoolean(),UseStepF2ForPumpMultiplayerCharts=item.GetProperty("StepF2").GetBoolean(),UseOutfoxFormatForSmx=item.GetProperty("Outfox").GetBoolean(),CustomProperties=item.GetProperty("Custom").GetBoolean()?custom:null,WriteAttacksFromFallbackChart=item.GetProperty("Target").GetString()=="sm"};saved=item.GetProperty("Target").GetString()=="sm"?new SMWriter(config).Save():new SSCWriter(config).Save();if(File.Exists(dst)){bytes=Convert.ToBase64String(File.ReadAllBytes(dst));after=Project(await Reader.CreateReader(dst).LoadAsync(CancellationToken.None));}}catch(Exception e){error=e.GetType().Name+": "+e.Message;}result.Add(new{Name=item.GetProperty("Name").GetString(),Saved=saved,Before=before,After=after,Bytes=bytes,Error=error});}Console.Write(JsonSerializer.Serialize(new{Cases=result}));}finally{Directory.Delete(folder,true);}
static object Project(Song song)=>song?.Charts.Select(chart=>new{Layout=chart.Type,Players=chart.NumPlayers,Columns=chart.NumInputs,Offset=chart.ChartOffsetFromMusic,Events=chart.Layers.SelectMany(l=>l.Events).Select(e=>new{Type=e.GetType().Name,Row=e.IntegerPosition,Player=e is LaneNote n?n.Player:-1,Lane=e is LaneNote m?m.Lane:-1,Source=e.SourceType,Dest=e.DestType}).ToArray()}).ToArray();
'''
subprocess.run([str(a.dotnet),'build',str(root/'Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
with tempfile.TemporaryDirectory(prefix='deadsync-layout-alias-build-')as folder:
 folder=Path(folder);(folder/'Program.cs').write_text(program);(folder/'request.json').write_text(json.dumps(request));(folder/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{root}/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(folder/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=folder/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly),str(folder/'request.json')],capture_output=True,text=True,check=True);expected=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');sha=lambda b:hashlib.sha256(b).hexdigest()
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=GA,editor_source_blobs={path:dict(git_blob=subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse',f'{GA}:{path}'],text=True).strip(),sha256=sha(data))for path,data in ga_blobs.items()},library_commit=LIB,fumen_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Actual Fumen full readers and writers. Writer flags only; GA pre-save guards and anonymization are reviewed separately, not executed.',source_blobs={path:dict(git_blob=subprocess.check_output(['git','-C',str(root),'rev-parse',f'{PIN}:{path}'],text=True).strip(),sha256=sha(data))for path,data in blobs.items()},program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,fumen_assembly_sha256=sha((root/'Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()),request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} real writer/reader cases')
