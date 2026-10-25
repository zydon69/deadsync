#!/usr/bin/env python3
"""Observe initialized rate state through unchanged original chart methods."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
ROOT = Path(__file__).resolve().parents[1]
LIB_PIN = 'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
FUMEN_PIN = '0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--editor-root',type=Path,required=True)
    parser.add_argument('--reference-root',type=Path,required=True)
    parser.add_argument('--dotnet',default='dotnet')
    args=parser.parse_args()
    for directory,pin in [(args.reference_root,LIB_PIN),(args.reference_root/'Fumen',FUMEN_PIN)]:
        actual=subprocess.check_output(['git','rev-parse','HEAD'],cwd=directory,text=True).strip()
        if actual!=pin: raise SystemExit('Reference checkout pin mismatch')
    project=ROOT/'scripts/editor-oracle'
    if not (project/'obj/PatternContextReference.cs').is_file():
        raise SystemExit('Prepare the existing editor oracle fixture workflow first')
    module=runpy.run_path(str(ROOT/'scripts/editor-generate-fixtures.py'))
    method_hash=module['prepare_timing_reference'](args.editor_root)
    env=dict(os.environ,DOTNET_CLI_TELEMETRY_OPTOUT='1',DOTNET_NOLOGO='1')
    subprocess.run([args.dotnet,'build',str(project/'EditorOracle.csproj'),'-c','Release',f'-p:ReferenceRoot={args.reference_root.resolve()}','--ignore-failed-sources'],check=True,env=env)
    fixtures=ROOT/'crates/deadsync-editor/tests/fixtures/grooveauthor'
    requests=sorted(fixtures.glob('spacing-sections-*.request.json'))
    if not requests: raise SystemExit('No spacing section requests')
    assembly=project/'bin/Release/net10.0/EditorOracle.dll'
    for path in requests:
        output=subprocess.check_output([args.dotnet,str(assembly),str(args.reference_root),str(path)],env=env)
        path.with_name(path.name.replace('.request.json','.expected.json')).write_bytes(output)
    # Accessor additions must not change prior behavior of the shared oracle.
    for name in ['editor-timing-baseline','editor-timing-warp','editor-timing-stacked-negative-stops']:
        request=fixtures/f'{name}.request.json'
        output=subprocess.check_output([args.dotnet,str(assembly),str(args.reference_root),str(request)],env=env)
        expected=json.loads((fixtures/f'{name}.expected.json').read_text())
        if json.loads(output)!=expected: raise SystemExit(f'Existing reference output changed: {name}')
    manifest={'GrooveAuthor':module['EDITOR_PIN'],'StepManiaLibrary':LIB_PIN,'Fumen':FUMEN_PIN,'OriginalTimingMethodsSHA256':method_hash,'AssembliesSHA256':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in [('Fumen',args.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll'),('StepManiaLibrary',args.reference_root/'StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll')]},'TimingAdapterSHA256':hashlib.sha256((project/'EditorTimingReference.cs.template').read_bytes()).hexdigest(),'Cases':[p.stem for p in requests],'Scope':'Unchanged RefreshRateAlteringEvents/RefreshEventTimingData; scalar init/update storage and getters; not full native editor'}
    (fixtures/'spacing-sections-reference.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Generated {len(requests)} original initialized rate-state traces')
if __name__=='__main__':main()
