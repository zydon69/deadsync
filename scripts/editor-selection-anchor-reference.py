#!/usr/bin/env python3
"""Execute the pinned original Selection class, without a replacement algorithm."""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--editor-root', type=Path, required=True)
p.add_argument('--dotnet', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
pin = '48e18f71655d0f0d86144c63239331833501fda2'
source = subprocess.check_output(['git', '-C', str(a.editor_root), 'show', pin + ':StepManiaEditor/Selection.cs'])
operations = ['S0', 'S2', 'S0', 'D2', 'D0', 'D3', 'S3', 'Clear', 'S1', 'S1', 'S2', 'D1', 'D2', 'S3', 'S0', 'Clear']
adapter = '''namespace StepManiaEditor;
internal class EditorEvent { public int Id; private bool selected;
public bool IsSelected()=>selected; public void Select()=>selected=true;
public void Deselect()=>selected=false; public bool IsAddedToChart()=>true; }
internal class EditorPatternEvent:EditorEvent {}
'''
program = '''using System.Text.Json; using StepManiaEditor;
var operations=JsonSerializer.Deserialize<string[]>(File.ReadAllText(args[0]));
var events=Enumerable.Range(0,4).Select(i=>new EditorEvent{Id=i}).ToArray();
var selection=new Selection();var steps=new List<object>();
foreach(var op in operations){
 if(op=="Clear")selection.ClearSelectedEvents();
 else {var e=events[int.Parse(op[1..])];if(op[0]=='S')selection.SelectEvent(e,true);else selection.DeselectEvent(e);}
 steps.Add(new {op,selected=selection.GetSelectedEvents().Select(e=>e.Id).ToArray(),last=(int?)selection.GetLastSelectedEvent()?.Id});
}
Console.Write(JsonSerializer.Serialize(new {steps}));
'''
with tempfile.TemporaryDirectory(prefix='editor-selection-anchor-') as directory:
    root = Path(directory)
    (root / 'Selection.cs').write_text(source.decode('utf-8-sig'))
    (root / 'Adapter.cs').write_text(adapter)
    (root / 'Program.cs').write_text(program)
    (root / 'request.json').write_text(json.dumps(operations))
    (root / 'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup></Project>')
    subprocess.run([str(a.dotnet), 'build', str(root / 'Oracle.csproj'), '-c', 'Release', '--nologo', '-v', 'quiet'], check=True)
    assembly = root / 'bin/Release/net10.0/Oracle.dll'
    result = subprocess.run([str(a.dotnet), str(assembly), str(root / 'request.json')], text=True, capture_output=True, check=True)
    expected = json.loads(result.stdout)
    assembly_hash = hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True, exist_ok=True)
a.output.write_text(json.dumps(expected, indent=2) + '\n')
a.output.with_suffix('.reference.json').write_text(json.dumps({
    'editor_commit': pin,
    'source_sha256': hashlib.sha256(source).hexdigest(),
    'assembly_sha256': assembly_hash,
    'scope': 'Complete original Selection class. Event adapter supplies only identity/selected flag. Sixteen actual select, reselect, deselect and clear operations; no GUI or region geometry is claimed.',
    'adapter_sha256': hashlib.sha256(adapter.encode()).hexdigest(),
    'program_sha256': hashlib.sha256(program.encode()).hexdigest(),
    'operations': operations,
}, indent=2) + '\n')
print('Executed original Selection:', len(expected['steps']), 'operations')
