#!/usr/bin/env python3
"""Probe actual pinned Fumen layout metadata and full note readers; temporary files only."""
import argparse,base64,hashlib,json,re,subprocess,tempfile
from pathlib import Path
LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.reference_root/'Fumen'
for folder,pin in [(a.reference_root,LIB),(root,PIN)]:
 assert subprocess.check_output(['git','-C',str(folder),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(folder),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
paths=['Fumen/Converters/SMCommon.cs','Fumen/Converters/SMPropertyParsers.cs','Fumen/Converters/SMReader.cs','Fumen/Converters/SSCReader.cs','Fumen/Converters/MSDFile.cs'];blobs={path:subprocess.check_output(['git','-C',str(root),'show',f'{PIN}:{path}'])for path in paths}
common=blobs[paths[0]].decode('utf-8-sig');enum=common.split('public enum ChartType')[1].split('}')[0];types=re.findall(r'^\s*([a-z][a-z0-9_]+),',enum,re.M);assert len(types)==43
cases=[]
def add(name,layout,fmt,notes='1000',full=False):
 text=('#VERSION:0.83;#TITLE:Alias;#MUSIC:music.ogg;#BPMS:0=120;'+(f'#NOTES:{layout}:keep:Hard:9::{notes};'if fmt=='sm' else f'#NOTEDATA:;#STEPSTYPE:{layout};#DESCRIPTION:keep;#DIFFICULTY:Hard;#METER:9;#NOTES:{notes};'))
 cases.append(dict(Name=name+'-'+fmt,Layout=layout,Format=fmt,Bytes=base64.b64encode(text.encode()).decode(),Full=full))
for index,kind in enumerate(types):
 for style,value in [('canonical',kind.replace('_','-')),('underscore',kind),('numeric',str(index))]:
  for fmt in ['sm','ssc']:add(f'type-{index}-{style}',value,fmt)
for alias,canonical in [('smx_double6','smx_dual'),('smx_double10','smx_full'),('smx_couple','smx_team'),('smx_routine','smx_team')]:
 for style,value in [('hyphen',alias.replace('_','-')),('underscore',alias),('padded',' \t'+alias+' \t'),('escaped-underscore',alias.replace('_','\\_')),('escaped-hyphen',alias.replace('_','\\-')),('comments','// before\n '+alias+' // after\n')]:
  for fmt in ['sm','ssc']:add('alias-'+alias+'-'+style,value,fmt)
 for value in [alias,alias.replace('_','-'),canonical]:
  columns=6 if canonical=='smx_dual' else 10;notes='1'+'0'*(columns-1)
  if canonical=='smx_team':notes+='&'+'0'*(columns-1)+'1'
  for fmt in ['sm','ssc']:add('full-'+alias+'-'+value,value,fmt,notes,True)
for name,value in [('unknown','future-layout'),('upper','SMX-DOUBLE6'),('empty',''),('out-of-range','999'),('negative','-1'),('combination','smx_single, smx_beginner'),('alias-combination','smx_double6,smx_double10'),('numeric-combination','38,39')]:
 for fmt in ['sm','ssc']:add('edge-'+name,value,fmt)
for kind,index in [('pump_double',8),('pump_routine',10)]:
 for style,value in [('canonical',kind.replace('_','-')),('underscore',kind),('numeric',str(index))]:
  for symbols in ['X000000000','XYZ1000000']:
   for fmt in ['sm','ssc']:add('full-'+kind+'-'+style+'-'+symbols,value,fmt,symbols,True)
request=dict(Cases=cases)
program='''using System;using System.Linq;using System.IO;using System.Text.Json;using System.Collections.Generic;using Fumen;using Fumen.Converters;using Fumen.ChartDefinition;using Path=System.IO.Path;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var folder=Path.Combine(Path.GetTempPath(),"deadsync-layout-alias-"+Guid.NewGuid());Directory.CreateDirectory(folder);var result=new List<object>();
try {foreach(var item in request.GetProperty("Cases").EnumerateArray()){var format=item.GetProperty("Format").GetString();var path=Path.Combine(folder,Guid.NewGuid()+"."+format);File.WriteAllBytes(path,Convert.FromBase64String(item.GetProperty("Bytes").GetString()));object metadata=null,full=null;string error=null;try{var reader=Reader.CreateReader(path);metadata=Project(await reader.LoadMetaDataAsync(CancellationToken.None));if(item.GetProperty("Full").GetBoolean())full=Project(await Reader.CreateReader(path).LoadAsync(CancellationToken.None));}catch(Exception e){error=e.GetType().Name;}result.Add(new{Name=item.GetProperty("Name").GetString(),Metadata=metadata,Full=full,Error=error});File.Delete(path);}Console.Write(JsonSerializer.Serialize(new{Cases=result}));}finally{Directory.Delete(folder,true);}
static object Project(Song song)=>song?.Charts.Select(chart=>new{Layout=chart.Type,Players=chart.NumPlayers,Columns=chart.NumInputs,Notes=chart.Layers.SelectMany(layer=>layer.Events).OfType<LaneNote>().Select(note=>new{Row=note.IntegerPosition,Lane=note.Lane,Player=note.Player}).ToArray()}).ToArray();
'''
subprocess.run([str(a.dotnet),'build',str(root/'Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
with tempfile.TemporaryDirectory(prefix='deadsync-layout-alias-build-')as folder:
 folder=Path(folder);(folder/'Program.cs').write_text(program);(folder/'request.json').write_text(json.dumps(request));(folder/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{root}/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(folder/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=folder/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly),str(folder/'request.json')],capture_output=True,text=True,check=True);expected=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');sha=lambda b:hashlib.sha256(b).hexdigest()
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(library_commit=LIB,fumen_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Actual Fumen metadata/full readers. Generated temporary simfiles only; no Editor or writers executed.',source_blobs={path:dict(git_blob=subprocess.check_output(['git','-C',str(root),'rev-parse',f'{PIN}:{path}'],text=True).strip(),sha256=sha(data))for path,data in blobs.items()},program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,fumen_assembly_sha256=sha((root/'Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()),request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} real-reader cases')
