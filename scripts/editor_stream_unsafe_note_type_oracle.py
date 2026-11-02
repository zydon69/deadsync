#!/usr/bin/env python3
"""Execute original pinned subdivision enum/helpers against actual Fumen; no loader emulation."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
PIN='48e18f71655d0f0d86144c63239331833501fda2'
def method(s,k):
 a=s.index(k);b=s.index('{',a);depth=1;i=b+1
 while depth:
  depth+=(s[i]=='{')-(s[i]=='}');i+=1
 return s[a:i]
def main():
 p=argparse.ArgumentParser();p.add_argument('--source',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 def raw(path):return subprocess.check_output(['git','-C',str(a.source),'show',PIN+':'+path])
 utils=raw('StepManiaEditor/Utils.cs');enum=raw('StepManiaEditor/SubdivisionType.cs');dll=a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
 host='using Fumen.Converters;using static Fumen.Converters.SMCommon;namespace StepManiaEditor {public static class Probe {'+'\n'.join(method(utils.decode('utf-8-sig'),k)for k in ['public static int GetBeatSubdivision(SubdivisionType subdivisionType)','public static int GetMeasureSubdivision(SubdivisionType subdivisionType)'])+'}}'
 program='using System.Text.Json;using StepManiaEditor;var rows=new List<object>();foreach(var raw in new[]{0,8,99,-99}){try{rows.Add(new{raw,result=(int?)Probe.GetMeasureSubdivision((SubdivisionType)raw),error=(string)null});}catch(Exception e){rows.Add(new{raw,result=(int?)null,error=e.GetType().Name});}}Console.Write(JsonSerializer.Serialize(rows));'
 with tempfile.TemporaryDirectory()as d:
  w=Path(d);(w/'SubdivisionType.cs').write_bytes(enum);(w/'Host.cs').write_text(host);(w/'Program.cs').write_text(program);(w/'Probe.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
  subprocess.run([str(a.dotnet),'build',str(w/'Probe.csproj'),'-c','Release','--nologo','-v','q'],check=True,capture_output=True)
  result=json.loads(subprocess.check_output([str(a.dotnet),str(w/'bin/Release/net10.0/Probe.dll')]))
 a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=PIN,utils_sha256=hashlib.sha256(utils).hexdigest(),enum_sha256=hashlib.sha256(enum).hexdigest(),fumen_sha256=hashlib.sha256(dll.read_bytes()).hexdigest(),host_sha256=hashlib.sha256(host.encode()).hexdigest(),scope=__doc__),indent=2)+'\n');print(result)
if __name__=='__main__':main()
