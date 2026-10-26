#!/usr/bin/env python3
"""Extract unchanged GrooveAuthor ChartComparer and record its observable order.
No graphics/native dependencies: the adapter supplies only the seven properties
read by the extracted comparator. Culture is the real .NET CurrentCulture.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--reference-root', type=Path, required=True)
parser.add_argument('--dotnet', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
source = args.reference_root / 'StepManiaEditor/EditorChart.cs'
text = source.read_text()
comparer = text[text.index('internal sealed class ChartComparer :'):]
# The comparator is the final class in the pinned source file.
assert comparer.rstrip().endswith('}')
types = ['dance-single','dance-double','dance-couple','dance-routine','dance-solo','dance-threepanel','pump-single','pump-halfdouble','pump-double','pump-couple','pump-routine','smx-beginner','smx-single','smx-dual','smx-full','smx-team']
keys = []
def add(**changes):
    key = dict(steps_type='dance-single', difficulty=3, rating=9, name='same', description='same', step_count=4, guid='11111111-2222-3333-4444-555555555555')
    key.update(changes)
    keys.append(key)
for kind in types + ['kb7-single', 'lights-cabinet']:
    add(steps_type=kind)
for difficulty in [0,1,2,4,5,77,-1]: add(difficulty=difficulty)
for rating in [-1,0,1,8,10,2147483647,-2147483648]: add(rating=rating)
for name in ['', 'éclair', 'eclair', 'Éclair', 'zebra', 'Ångström', '-start', 'start', 'a\u0301', 'á']:
    add(name=name)
for desc in ['', 'éclair', 'Éclair', 'zebra']: add(description=desc)
for count in [0,3,5]: add(step_count=count)
for guid in ['00000000-0000-0000-0000-000000000000','80000000-0000-0000-0000-000000000000','11111111-2222-3333-4444-555555555556']:
    add(guid=guid)
keys += [keys[0].copy(), None]
strings = sorted({key[field] for key in keys if key for field in ['name','description']})
request = dict(keys=keys, strings=strings)
program = '''using System; using System.Linq; using System.Collections.Generic; using System.Globalization; using System.Text.Json;
enum ChartType { ENUM }
sealed class EditorChart {
 public ChartType ChartType; public int ChartDifficultyType; public int Rating; public string Name; public string Description; public int Steps; public Guid Id;
 public EditorChart(JsonElement e) { ChartType=Enum.Parse<ChartType>(e.GetProperty("steps_type").GetString().Replace('-','_')); ChartDifficultyType=e.GetProperty("difficulty").GetInt32(); Rating=e.GetProperty("rating").GetInt32(); Name=e.GetProperty("name").GetString(); Description=e.GetProperty("description").GetString(); Steps=e.GetProperty("step_count").GetInt32(); Id=e.GetProperty("guid").GetGuid(); }
 public EditorChart GetStepTotals()=>this; public int GetStepCount()=>Steps; public Guid GetGuid()=>Id;
}
class Program { static void Main() {
 var input=JsonDocument.Parse(Console.In.ReadToEnd()).RootElement;
 var keys=input.GetProperty("keys").EnumerateArray().Select(e=>e.ValueKind==JsonValueKind.Null?null:new EditorChart(e)).ToArray();
 var strings=input.GetProperty("strings").EnumerateArray().Select(e=>e.GetString()).ToArray();
 var cultures=new[]{"en-US","fr-FR","sv-SE"}.Select(culture=>{
  CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);
  return new { culture, comparisons=keys.Select(a=>keys.Select(b=>Math.Sign(ChartComparer.Compare(a,b))).ToArray()).ToArray(), text_comparisons=strings.Select(a=>strings.Select(b=>Math.Sign(string.Compare(a,b,StringComparison.CurrentCulture))).ToArray()).ToArray() };
 }).ToArray();
 Console.Write(JsonSerializer.Serialize(new {cultures}));
} }
'''.replace('ENUM', ','.join(t.replace('-','_') for t in types + ['kb7-single','lights-cabinet']))
with tempfile.TemporaryDirectory(prefix='deadsync-chart-order-oracle-') as folder:
    root = Path(folder)
    (root/'Reference.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><CheckForOverflowUnderflow>false</CheckForOverflowUnderflow></PropertyGroup></Project>')
    (root/'Program.cs').write_text(program + '\n' + comparer)
    subprocess.run([str(args.dotnet),'build',str(root/'Reference.csproj'),'--configuration','Release','--nologo','--verbosity','quiet'], check=True)
    output=subprocess.run([str(args.dotnet),str(root/'bin/Release/net10.0/Reference.dll')],input=json.dumps(request),text=True,capture_output=True,check=True)
    result=json.loads(output.stdout)
result.update(request)
result['provenance'] = dict(reference_commit=subprocess.check_output(['git','-C',str(args.reference_root),'rev-parse','HEAD'],text=True).strip(), source='StepManiaEditor/EditorChart.cs', source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(), extracted_comparer_sha256=hashlib.sha256(comparer.encode()).hexdigest(), scope='Unchanged ChartComparer with property-only EditorChart adapter. Actual .NET CurrentCulture strings. Does not prove native collation adapter or EditorSong UI lifecycle.')
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(f'Wrote {len(keys)} chart keys, {len(keys)**2*len(result["cultures"])} comparisons: {args.output}')
