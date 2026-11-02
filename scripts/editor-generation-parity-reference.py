#!/usr/bin/env python3
"""Execute original graph search, preserve every result and compare old fixtures.
Does not execute Rust or infer algorithm parity from successful source execution.
"""
import argparse,hashlib,json,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--assembly',type=Path);p.add_argument('--repo',type=Path,required=True);p.add_argument('--library',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
pins={'library':'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8','fumen':'0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'}
for root,pin in [(a.library,pins['library']),(a.library/'Fumen',pins['fumen'])]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
fixture=a.repo/'crates/deadsync-editor/tests/fixtures/grooveauthor';assembly=a.assembly or a.repo/'scripts/editor-oracle/bin/Release/net10.0/EditorOracle.dll';a.output.mkdir(parents=True,exist_ok=True)
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
results=[]
for path in sorted(fixture.glob('*.request.json')):
 request=json.loads(path.read_text())
 if not isinstance(request,dict):continue
 op=request.get('Operation');name=path.name.removesuffix('.request.json')
 if not ((op=='InterpretChart' and (name.startswith('chart-') or name.startswith('policy-'))) or (op=='Pattern' and name.startswith('layout-')) or op=='Convert'):continue
 start=time.monotonic();run=subprocess.run([str(a.dotnet),str(assembly),str(a.library),str(path)],capture_output=True,text=True,timeout=120)
 if run.returncode:raise RuntimeError(name+': '+run.stderr)
 actual=json.loads(run.stdout);expected=path.with_name(name+'.expected.json');equal=actual==json.loads(expected.read_text())
 (a.output/(name+'.actual.json')).write_text(json.dumps(actual,indent=2)+'\n')
 results.append(dict(name=name,operation=op,layout=request.get('Layout'),request_sha256=sha(path),expected_sha256=sha(expected),matches_committed_source=equal,success=actual.get('Success') if isinstance(actual,dict) else None,seconds=round(time.monotonic()-start,3)))
 (a.output/'results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name, 'same' if equal else 'DIFF',flush=True)
(a.output/'provenance.json').write_text(json.dumps(dict(pins=pins,assembly_sha256=sha(assembly),program_sha256=sha(a.repo/'scripts/editor-oracle/Program.cs'),results=len(results),notice='Actual original .NET search executed. Committed source comparison is not a Rust execution. Parameter mutation coverage and 11-layout Rust validation are separate.'),indent=2)+'\n')
