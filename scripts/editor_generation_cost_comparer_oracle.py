#!/usr/bin/env python3
"""Execute the unchanged pinned SearchNode.CompareTo, with field-only plumbing."""
import argparse, hashlib, json, re, subprocess, tempfile
from pathlib import Path

PIN = 'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
SOURCE = 'StepManiaLibrary/PerformedChart/SearchNode.cs'
p = argparse.ArgumentParser()
p.add_argument('--reference-root', type=Path, required=True)
p.add_argument('--dotnet', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
raw = subprocess.check_output(['git', '-C', str(a.reference_root), 'show', f'{PIN}:{SOURCE}'])
s = raw.decode('utf-8-sig')
start = s.index('public int CompareTo(SearchNode other)')
end = s.index('{', start) + 1
depth = 1
while depth:
    depth += (s[end] == '{') - (s[end] == '}')
    end += 1
method = s[start:end]
fields = [(t, n) for t, n in re.findall(r'private (?:readonly )?(double|float|int) (\w+);', s)
          if re.search(r'\b' + n + r'\b', method)]
program = '''using System;using System.Collections.Generic;using System.Text.Json;
class SearchNode {
FIELDS
METHOD
static void Main(){var rows=new List<object>();
int[] bits={0,722255052,925353387,925353388,925353389,1065353216};
foreach(var field in new[]{"pattern_type","distribution"})
foreach(var x in bits)foreach(var y in bits)foreach(var reverse in new[]{false,true}){
var a=new SearchNode{RandomWeight=reverse?0.9:0.1};var b=new SearchNode{RandomWeight=reverse?0.1:0.9};
if(field=="pattern_type"){a.PatternGenerationStepTypeCost=BitConverter.Int32BitsToSingle(x);b.PatternGenerationStepTypeCost=BitConverter.Int32BitsToSingle(y);}
else{a.TotalDistributionCost=BitConverter.Int32BitsToSingle(x);b.TotalDistributionCost=BitConverter.Int32BitsToSingle(y);}
rows.Add(new{field,x,y,reverse,expected=Math.Sign(a.CompareTo(b))});}
Console.Write(JsonSerializer.Serialize(rows));}}
'''.replace('FIELDS', '\n'.join(f'public {t} {n};' for t, n in fields)).replace('METHOD', method)
with tempfile.TemporaryDirectory(prefix='deadsync-cost-comparer-') as folder:
    root = Path(folder)
    (root / 'Program.cs').write_text(program)
    (root / 'Oracle.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><NoWarn>0649</NoWarn></PropertyGroup></Project>')
    subprocess.run([str(a.dotnet), 'build', str(root / 'Oracle.csproj'), '-c', 'Release', '--nologo', '-v', 'quiet'], check=True)
    result = json.loads(subprocess.check_output([str(a.dotnet), str(root / 'bin/Release/net10.0/Oracle.dll')]))
a.output.parent.mkdir(parents=True, exist_ok=True)
a.output.write_text(json.dumps(result, indent=2) + '\n')
a.output.with_suffix('.provenance.json').write_text(json.dumps({
    'LibraryCommit': PIN, 'SourcePath': SOURCE,
    'SourceSha256': hashlib.sha256(raw).hexdigest(),
    'UnchangedCompareToSha256': hashlib.sha256(method.encode()).hexdigest(),
    'HarnessSha256': hashlib.sha256(program.encode()).hexdigest(),
    'FixtureSha256': hashlib.sha256(a.output.read_bytes()).hexdigest(),
    'Cases': len(result),
    'Scope': 'Actual unchanged original CompareTo. Field-only SearchNode plumbing with original declared numeric types; construction/scoring/search and full-chart output not executed. Nonnegative finite f32 bit patterns, both f32 cost fields, both random tie-break directions.',
}, indent=2) + '\n')
print(f'Executed {len(result)} original cost comparisons.')
