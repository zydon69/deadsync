#!/usr/bin/env python3
"""One-leaf source search mutations; unchanged output is NOT an exercised proof."""
import argparse,copy,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--assembly',type=Path);p.add_argument('--repo',type=Path,required=True);p.add_argument('--library',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();fixtures=a.repo/'crates/deadsync-editor/tests/fixtures/grooveauthor';assembly=a.assembly or a.repo/'scripts/editor-oracle/bin/Release/net10.0/EditorOracle.dll';a.output.mkdir(parents=True,exist_ok=True)
for root,pin in [(a.library,'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'),(a.library/'Fumen','0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89')]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
def leaves(value,path=()):
 if isinstance(value,dict):
  for key,child in value.items():yield from leaves(child,path+(key,))
 elif isinstance(value,list):
  for key,child in enumerate(value):yield from leaves(child,path+(key,))
 else:yield path,value
def changed(value,path):
 if isinstance(value,bool):return not value
 if isinstance(value,str):return {'balanced':'aggressive','chooseMethodDynamically':'useDefaultMethod','automatic':'random','automaticSameOrNewLane':'specifiedLane','automaticSameOrNewLaneAsFollowing':'specifiedLane'}[value]
 if path[-1]=='BeatSubDivision':return 2
 if isinstance(value,int):return 1 if value==0 else max(0,value-1)
 return value*0.9 if value!=0 else 0.05
matrix=[]
for family,seed,group in [('expression','policy-balanced','ExpressedConfig'),('pattern','layout-dance-single','PatternConfig'),('performed','layout-dance-double','PerformedConfig')]:
 baseline=json.loads((fixtures/(seed+'.request.json')).read_text());base_output=json.loads((fixtures/(seed+'.expected.json')).read_text())
 for path,value in leaves(baseline[group]):
  request=copy.deepcopy(baseline)
  # ArrowWeights must be evaluated on their actual layout, not an unused map entry.
  if path[0]=='ArrowWeights':
   layout=path[1];request=json.loads((fixtures/('layout-'+layout+'.request.json')).read_text());base_output=json.loads((fixtures/('layout-'+layout+'.expected.json')).read_text())
  else:base_output=json.loads((fixtures/(seed+'.expected.json')).read_text())
  cursor=request[group]
  for key in path[:-1]:cursor=cursor[key]
  after=changed(value,path);cursor[path[-1]]=after
  name=('policy-parity-' if family=='expression' else 'parity-'+family+'-')+'-'.join(map(str,path)).lower();rp=a.output/(name+'.request.json');rp.write_text(json.dumps(request,indent=2)+'\n')
  run=subprocess.run([str(a.dotnet),str(assembly),str(a.library),str(rp)],capture_output=True,text=True,timeout=120)
  row=dict(name=name,family=family,path=list(path),before=value,after=after,layout=request['Layout'])
  if run.returncode:row.update(status='source_refused',diagnostic=run.stderr)
  else:
   output=json.loads(run.stdout);(a.output/(name+'.expected.json')).write_text(json.dumps(output,indent=2)+'\n');row.update(status='source_executed',success=output.get('Success'),observable_change=output!=base_output)
  matrix.append(row);(a.output/'matrix.json').write_text(json.dumps(matrix,indent=2)+'\n');print(name,row['status'],row.get('observable_change'),flush=True)
(a.output/'provenance.json').write_text(json.dumps(dict(assembly_sha256=hashlib.sha256(assembly.read_bytes()).hexdigest(),mutations=len(matrix),notice='Original source search executed once per leaf. No Rust execution. observable_change=false means no demonstrated effect for this witness, never parity coverage.'),indent=2)+'\n')
