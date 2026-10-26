#!/usr/bin/env python3
"""Test JSON uint compatibility of the original minimap range declarations.
The isolated DTO copies two source declarations only, not the application loader.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
PIN = "48e18f71655d0f0d86144c63239331833501fda2"
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-root',type=Path,required=True)
    parser.add_argument('--dotnet',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    commit=subprocess.check_output(['git','-C',str(args.reference_root),'rev-parse','HEAD'],text=True).strip()
    if commit != PIN: raise SystemExit('Wrong reference commit')
    source_path=args.reference_root/'StepManiaEditor/Preferences/PreferencesMiniMap.cs'
    source_bytes=source_path.read_bytes()
    pinned=subprocess.check_output(['git','-C',str(args.reference_root),'show',f'{PIN}:StepManiaEditor/Preferences/PreferencesMiniMap.cs'])
    if source_bytes != pinned: raise SystemExit('Reference working tree differs from pinned source')
    source=source_bytes.decode('utf-8-sig')
    for name in ['MiniMapVisibleTimeRange','MiniMapVisibleRowRange']:
        if f'[JsonInclude] public uint {name} =' not in source: raise SystemExit('Source declaration changed')
    cases=[{'MiniMapVisibleTimeRange':240,'MiniMapVisibleRowRange':24576},
           {'MiniMapVisibleTimeRange':120,'MiniMapVisibleRowRange':384},
           {'MiniMapVisibleTimeRange':4294967295,'MiniMapVisibleRowRange':1},
           {'MiniMapVisibleTimeRange':240.0,'MiniMapVisibleRowRange':24576.0}]
    program='''using System;using System.Text.Json;using System.Text.Json.Serialization;
class RangePreferences {
 [JsonInclude] public uint MiniMapVisibleTimeRange;
 [JsonInclude] public uint MiniMapVisibleRowRange;
}
class Program { static void Main() {
 var cases=JsonSerializer.Deserialize<JsonElement[]>(Console.In.ReadToEnd());
 var results=Array.ConvertAll(cases,input=> {
  try { var value=JsonSerializer.Deserialize<RangePreferences>(input.GetRawText());
   return new { accepted=true, time=value.MiniMapVisibleTimeRange, row=value.MiniMapVisibleRowRange };
  } catch(JsonException) { return new {accepted=false,time=0u,row=0u}; }
 }); Console.Write(JsonSerializer.Serialize(results));
}}
'''
    with tempfile.TemporaryDirectory(prefix='deadsync-minimap-preferences-') as folder:
        root=Path(folder)
        (root/'oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework></PropertyGroup></Project>')
        (root/'Program.cs').write_text(program)
        subprocess.run([str(args.dotnet),'build',str(root/'oracle.csproj'),'--nologo','-v','q'],capture_output=True,check=True)
        result=subprocess.run([str(args.dotnet),str(root/'bin/Debug/net10.0/oracle.dll')],input=json.dumps(cases),text=True,capture_output=True,check=True)
    expected=json.loads(result.stdout)
    assert [r['accepted'] for r in expected] == [True,True,True,False]
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(dict(reference_commit=commit,source=str(source_path.relative_to(args.reference_root)),source_sha256=hashlib.sha256(source_path.read_bytes()).hexdigest(),scope='System.Text.Json uint range DTO only; original full preference loader is not executed.',cases=[dict(input=i,expected=e) for i,e in zip(cases,expected)]),indent=2)+'\n')
    print('Validated 3 integer ranges and rejection of decimal JSON tokens via .NET uint DTO')
if __name__=='__main__':main()
