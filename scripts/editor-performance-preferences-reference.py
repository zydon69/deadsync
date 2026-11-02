#!/usr/bin/env python3
"""Run original Performance preferences, converter and Restore/Undo/Redo."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

p = argparse.ArgumentParser()
for key in ['editor-root', 'reference-root', 'dotnet', 'output']:
    p.add_argument('--' + key, type=Path, required=True)
a = p.parse_args()
pin = '48e18f71655d0f0d86144c63239331833501fda2'
fumen_pin = '0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for root, revision in [(a.editor_root,pin),(a.reference_root/'Fumen',fumen_pin)]:
    assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip() == revision
source_bytes = subprocess.check_output(['git','-C',str(a.editor_root),'show',
    pin + ':StepManiaEditor/Preferences/PreferencesPerformance.cs'])
dll = a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'
adapter = r'''
using System.Text.Json;
using Fumen;
using StepManiaEditor;
namespace StepManiaEditor {
 class Preferences {
  public static Preferences Instance = new();
  public PreferencesPerformance PreferencesPerformance = new();
 }
}
class Program {
 static void Main() {
  var converter=new PermissiveEnumJsonConverterFactory();
  PreferencesPerformance.RegisterDefaultsForInvalidEnumValues(converter);
  var options=new JsonSerializerOptions{IncludeFields=true};
  options.Converters.Add(converter);
  var output=new List<object>();
  foreach(var json in new[]{"{}","{\"ShowPerformanceWindow\":true}",
   "{\"FrameMaxTime\":99}","{\"FrameMaxTime\":\"99\"}",
   "{\"FrameMaxTime\":\"future\"}","{\"FrameMaxTime\":\"Shared, Explicit\"}",
   "{\"FrameMaxTime\":null}","{\"FrameMaxTime\":true}",
   "{\"MaxFramesToDraw\":-3,\"ExplicitFrameMaxTime\":-0.5}",
   "{\"ExplicitFrameMaxTime\":0.01666666666666667}",
   "{\"PerformanceMonitorPaused\":true,\"MaxFramesToDraw\":1024}"}) {
   var value=JsonSerializer.Deserialize<PreferencesPerformance>(json,options);
   Preferences.Instance.PreferencesPerformance=value;
   var initial=JsonSerializer.SerializeToElement(value,options);
   var defaults=value.IsUsingDefaults();
   value.MaxFramesToDraw=128;value.FrameMaxTime=PreferencesPerformance.FrameMaxTimeMode.Explicit;
   value.ExplicitFrameMaxTime=.01;value.PerformanceMonitorPaused=true;
   var changed=JsonSerializer.SerializeToElement(value,options);
   value.RestoreDefaults();var restored=JsonSerializer.SerializeToElement(value,options);
   ActionQueue.Instance.Undo();var undone=JsonSerializer.SerializeToElement(value,options);
   ActionQueue.Instance.Redo();var redone=JsonSerializer.SerializeToElement(value,options);
   output.Add(new{Input=JsonDocument.Parse(json).RootElement.Clone(),Initial=initial,
    UsingDefaults=defaults,Changed=changed,Restored=restored,Undone=undone,Redone=redone});
   ActionQueue.Instance.Clear();
  }
  Console.Write(JsonSerializer.Serialize(output));
 }
}
'''
with tempfile.TemporaryDirectory(prefix='ga-performance-preferences-') as temp:
    root=Path(temp)
    (root/'PreferencesPerformance.cs').write_bytes(source_bytes)
    (root/'Program.cs').write_text(adapter)
    (root/'Reference.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{dll}</HintPath></Reference></ItemGroup></Project>')
    subprocess.run([str(a.dotnet),'build',str(root/'Reference.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
    output=json.loads(subprocess.check_output([str(a.dotnet),str(root/'bin/Release/net10.0/Reference.dll')],text=True))
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps(dict(revision=pin,fumen_revision=fumen_pin,
    source_sha256=hashlib.sha256(source_bytes).hexdigest(),
    adapter_sha256=hashlib.sha256(adapter.encode()).hexdigest(),
    scope='Whole original preferences, actual Fumen converter and ActionQueue; no native UI or recorder execution',
    cases=output),indent=2)+'\n')
