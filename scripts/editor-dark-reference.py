#!/usr/bin/env python3
"""Run the pinned original Dark preferences class and its real Fumen undo action."""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--editor-root', type=Path, required=True)
parser.add_argument('--reference-root', type=Path, required=True)
parser.add_argument('--dotnet', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
pin = '48e18f71655d0f0d86144c63239331833501fda2'
fumen_pin = '0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for directory, revision in [(args.editor_root, pin), (args.reference_root / 'Fumen', fumen_pin)]:
    assert subprocess.check_output(['git', '-C', str(directory), 'rev-parse', 'HEAD'], text=True).strip() == revision
source = subprocess.check_output(['git', '-C', str(args.editor_root), 'show', f'{pin}:StepManiaEditor/Preferences/PreferencesDark.cs'])
adapter = '''using System.Text.Json;using System.Numerics;using Fumen;using Fumen.Converters;using StepManiaEditor;
namespace StepManiaEditor {class Preferences {public static Preferences Instance=new();public PreferencesDark PreferencesDark=new();}}
class Program {static void Main(){var factory=new PermissiveEnumJsonConverterFactory();PreferencesDark.RegisterDefaultsForInvalidEnumValues(factory);var options=new JsonSerializerOptions{IncludeFields=true,Converters={factory}};var cases=new List<object>();foreach(var json in new[]{"{}","{\\"ShowDarkBg\\":true,\\"Size\\":\\"Charts\\",\\"DarkBgDrawOrder\\":\\"AfterWaveForm\\",\\"Color\\":{\\"X\\":0.2,\\"Y\\":0.4,\\"Z\\":0.6,\\"W\\":0.3}}","{\\"Size\\":\\"Future\\",\\"DarkBgDrawOrder\\":999}","{\\"Color\\":{\\"X\\":1}}","{\\"Color\\":{\\"X\\":-2,\\"Y\\":3,\\"Z\\":0.5,\\"W\\":0.8}}"}){var p=JsonSerializer.Deserialize<PreferencesDark>(json,options);Preferences.Instance.PreferencesDark=p;var normalized=JsonSerializer.SerializeToElement(p,options);p.ShowDarkBg=true;p.Size=PreferencesDark.SizeMode.Charts;p.DarkBgDrawOrder=PreferencesDark.DrawOrder.AfterWaveForm;p.Color=new Vector4(0.2f,0.4f,0.6f,0.3f);var changed=JsonSerializer.SerializeToElement(p,options);p.RestoreDefaults();var restored=JsonSerializer.SerializeToElement(p,options);ActionQueue.Instance.Undo();var undone=JsonSerializer.SerializeToElement(p,options);ActionQueue.Instance.Redo();var redone=JsonSerializer.SerializeToElement(p,options);cases.Add(new{Input=JsonDocument.Parse(json).RootElement.Clone(),Normalized=normalized,Changed=changed,Restored=restored,Undone=undone,Redone=redone});ActionQueue.Instance.Clear();}Console.Write(JsonSerializer.Serialize(cases));}}
'''
with tempfile.TemporaryDirectory(prefix='deadsync-dark-source-') as directory:
    root = Path(directory)
    (root / 'Dark.cs').write_text(source.decode('utf-8-sig'))
    (root / 'Program.cs').write_text(adapter)
    (root / 'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{args.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference></ItemGroup></Project>')
    subprocess.run([str(args.dotnet), 'build', str(root / 'Oracle.csproj'), '-c', 'Release', '--nologo', '-v', 'quiet'], check=True)
    assembly = root / 'bin/Release/net10.0/Oracle.dll'
    result = json.loads(subprocess.check_output([str(args.dotnet), str(assembly)], text=True))
    assembly_hash = hashlib.sha256(assembly.read_bytes()).hexdigest()
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(result, indent=2) + '\n')
args.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit': pin, 'FumenCommit': fumen_pin, 'SourceSha256': hashlib.sha256(source).hexdigest(), 'AdapterSha256': hashlib.sha256(adapter.encode()).hexdigest(), 'AssemblySha256': assembly_hash, 'Scope': 'Whole original PreferencesDark class and original RestoreDefaults action, real Fumen converter and undo queue. GPU rasterization and MonoGame color conversion not executed.'}, indent=2) + '\n')
print(f'Executed {len(result)} source Dark preference/history cases.')
