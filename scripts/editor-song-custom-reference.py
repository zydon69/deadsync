#!/usr/bin/env python3
"""Probe original Fumen custom-song aliases and precedence; temporary files only."""
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
def pair(suffix,offset,version='1'):
 data= json.dumps(dict(SyncOffset=offset,Future=dict(Keep=True)),separators=(',',':')).replace(':','\\:')
 return f'#GASongDataVersion{suffix}:{version};#GASongData{suffix}:{data};'
variants=[('none',''),('plain',pair('',0.1)),('marked',pair('SONG',0.2)),('plain-then-marked',pair('',0.1)+pair('SONG',0.2)),('marked-then-plain',pair('SONG',0.2)+pair('',0.1)),('marked-duplicate',pair('SONG',0.2)+pair('SONG',0.3)),('mixed-version',pair('SONG',0.2)+'#GASongDataVersion:99;'),('mixed-data',pair('SONG',0.2)+'#GASongData:{"SyncOffset"\\:0.4};'),('lower-marked',pair('song',0.2)),('lower-base',pair('',0.2).replace('GASongData','gasongdata')),('version-only','#GASongDataVersionSONG:1;'),('marked-old',pair('SONG',0.2,'99'))]
for name,tags in variants:
 for fmt in ['sm','ssc']:
  chart='#NOTES:dance-single::Hard:9::1000;' if fmt=='sm'else'#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;'
  text='#VERSION:0.83;#BPMS:0=120;'+tags+chart
  cases.append(dict(Name=name+'-'+fmt,Format=fmt,Bytes=base64.b64encode(text.encode()).decode()))
request=dict(Cases=cases)
program='''using System;using System.Linq;using System.IO;using System.Text.Json;using System.Collections.Generic;using Fumen;using Fumen.Converters;using Fumen.ChartDefinition;using Path=System.IO.Path;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var folder=Path.Combine(Path.GetTempPath(),"deadsync-song-custom-"+Guid.NewGuid());Directory.CreateDirectory(folder);var result=new List<object>();try{foreach(var item in request.GetProperty("Cases").EnumerateArray()){var path=Path.Combine(folder,Guid.NewGuid()+"."+item.GetProperty("Format").GetString());File.WriteAllBytes(path,Convert.FromBase64String(item.GetProperty("Bytes").GetString()));var song=await Reader.CreateReader(path).LoadMetaDataAsync(CancellationToken.None);song.Extras.TryGetExtra("GASongDataVersion",out string version,true);song.Extras.TryGetExtra("GASongData",out string data,true);double? offset=null;string error=null;if(version=="1"&&data!=null){try{var json=JsonDocument.Parse(data);offset=json.RootElement.TryGetProperty("SyncOffset",out var sync)?sync.GetDouble():0;}catch(Exception e){error=e.GetType().Name;}}result.Add(new{Name=item.GetProperty("Name").GetString(),Version=version,Data=data,Offset=offset,Error=error});}Console.Write(JsonSerializer.Serialize(new{Cases=result}));}finally{Directory.Delete(folder,true);}
'''
subprocess.run([str(a.dotnet),'build',str(root/'Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
with tempfile.TemporaryDirectory(prefix='deadsync-layout-alias-build-')as folder:
 folder=Path(folder);(folder/'Program.cs').write_text(program);(folder/'request.json').write_text(json.dumps(request));(folder/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{root}/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(folder/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=folder/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly),str(folder/'request.json')],capture_output=True,text=True,check=True);expected=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');sha=lambda b:hashlib.sha256(b).hexdigest()
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=GA,editor_source_blobs={path:dict(git_blob=subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse',f'{GA}:{path}'],text=True).strip(),sha256=sha(data))for path,data in ga_blobs.items()},library_commit=LIB,fumen_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Actual Fumen metadata reader custom properties. Raw effective extras and V1 SyncOffset projection; GA constructor and writes not executed.',source_blobs={path:dict(git_blob=subprocess.check_output(['git','-C',str(root),'rev-parse',f'{PIN}:{path}'],text=True).strip(),sha256=sha(data))for path,data in blobs.items()},program_sha256=sha(program.encode()),assembly_sha256=assembly_hash,fumen_assembly_sha256=sha((root/'Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()),request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} real writer/reader cases')
