#!/usr/bin/env python3
"""Run actual CreateFromSMEvents -> CreateFromExpressedChart on all 121 pairs."""
import argparse,copy,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',type=Path,required=True);p.add_argument('--library',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--assembly',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();fixtures=a.repo/'crates/deadsync-editor/tests/fixtures/grooveauthor';a.output.mkdir(parents=True,exist_ok=True)
for root,pin in [(a.library,'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'),(a.library/'Fumen','0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89')]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
base=json.loads((fixtures/'convert-dance-single-to-dance-double-42.request.json').read_text());layouts=list(base['PerformedConfig']['ArrowWeights']);matrix=json.loads((a.output/'matrix.json').read_text()) if (a.output/'matrix.json').exists() else []
completed={row['name']for row in matrix}
for source in layouts:
 notes=json.loads((fixtures/('chart-'+source+'-balanced.request.json')).read_text())['Notes']
 for target in layouts:
  name='parity-pair-'+source+'-to-'+target
  if name in completed:continue
  request=copy.deepcopy(base);request.update(Layout=target,SourceLayout=source,Notes=notes);path=a.output/(name+'.request.json');path.write_text(json.dumps(request,indent=2)+'\n');run=subprocess.run([str(a.dotnet),str(a.assembly),str(a.library),str(path)],capture_output=True,text=True,timeout=120)
  if run.returncode:
   (a.output/(name+'.source-exception.txt')).write_text(run.stderr);matrix.append(dict(name=name,source=source,target=target,status='source_exception',success=None));(a.output/'matrix.json').write_text(json.dumps(matrix,indent=2)+'\n');print(name,'SOURCE_EXCEPTION',flush=True);continue
  expected=json.loads(run.stdout);(a.output/(name+'.expected.json')).write_text(json.dumps(expected,indent=2)+'\n');matrix.append(dict(name=name,source=source,target=target,success=expected['Success'],stage=expected.get('Stage')));(a.output/'matrix.json').write_text(json.dumps(matrix,indent=2)+'\n');print(name,expected['Success'],flush=True)
(a.output/'provenance.json').write_text(json.dumps(dict(assembly_sha256=hashlib.sha256(a.assembly.read_bytes()).hexdigest(),pairs=len(matrix),scope='One seeded source chart per source layout, seed42, default fallback. Original algorithms actually executed. Does not certify all charts, seeds, parameters or Rust.'),indent=2)+'\n')
