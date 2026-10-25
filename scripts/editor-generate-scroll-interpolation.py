#!/usr/bin/env python3
"""Compose actual speed-event timestamps with unchanged C# interpolation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[1]
LIB_PIN='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
FUMEN_PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--editor-root',type=Path,required=True)
    parser.add_argument('--reference-root',type=Path,required=True)
    parser.add_argument('--dotnet',default='dotnet')
    args=parser.parse_args()
    for directory,pin in [(args.reference_root,LIB_PIN),(args.reference_root/'Fumen',FUMEN_PIN)]:
        if subprocess.check_output(['git','rev-parse','HEAD'],cwd=directory,text=True).strip()!=pin:raise SystemExit('Reference pin mismatch')
    project=ROOT/'scripts/editor-oracle'
    if not (project/'obj/PatternContextReference.cs').is_file():raise SystemExit('Prepare the existing editor oracle workflow first')
    module=runpy.run_path(str(ROOT/'scripts/editor-generate-fixtures.py'))
    timing_hash=module['prepare_timing_reference'](args.editor_root)
    speed_hash=module['prepare_speed_reference'](args.editor_root)
    env=dict(os.environ,DOTNET_NOLOGO='1',DOTNET_CLI_TELEMETRY_OPTOUT='1')
    subprocess.run([args.dotnet,'build',str(project/'EditorOracle.csproj'),'-c','Release',f'-p:ReferenceRoot={args.reference_root.resolve()}','--ignore-failed-sources'],check=True,env=env)
    assembly=project/'bin/Release/net10.0/EditorOracle.dll'
    def run(value):
        with tempfile.TemporaryDirectory(prefix='deadsync-scroll-reference-') as directory:
            path=Path(directory)/'request.json';path.write_text(json.dumps(value))
            return json.loads(subprocess.check_output([args.dotnet,str(assembly),str(args.reference_root),str(path)],env=env))
    fixtures=ROOT/'crates/deadsync-editor/tests/fixtures/grooveauthor'
    requests=sorted(fixtures.glob('scroll-interpolation-*.request.json'))
    if not requests:raise SystemExit('No interpolation cases found')
    for path in requests:
        request=json.loads(path.read_text())
        origins=run({'Operation':'EditorTiming','SpeedOrigins':True,'Timing':request['Timing']+[[row,'speed'] for row in request['SpeedRows']],'Notes':[]})
        speeds=run({'Operation':'Speed','Source':request['Source'],'Queries':request['Queries'],'Operations':[],'EventTimes':{str(origin['Row']):origin['Time'] for origin in origins}})
        path.with_name(path.name.replace('.request.json','.expected.json')).write_text(json.dumps({'Origins':origins,'Speeds':speeds},indent=2)+'\n')
    for name in ['speed-edit-seconds','speed-edit-rows-and-interruption','editor-timing-baseline']:
        expected=json.loads((fixtures/f'{name}.expected.json').read_text())
        if run(json.loads((fixtures/f'{name}.request.json').read_text()))!=expected:raise SystemExit(f'Existing oracle output changed: {name}')
    manifest={'GrooveAuthor':module['EDITOR_PIN'],'StepManiaLibrary':LIB_PIN,'Fumen':FUMEN_PIN,'TimingMethodsSHA256':timing_hash,'SpeedMethodsSHA256':speed_hash,'AdapterSHA256':{name:hashlib.sha256((project/name).read_bytes()).hexdigest() for name in ['EditorTimingReference.cs.template','SpeedReference.cs.template']},'Cases':[path.stem for path in requests],'Scope':'Original event-timing pass supplies speed origins to unchanged interpolation methods; row selection adapters; not full native display'}
    (fixtures/'scroll-interpolation-reference.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Generated {len(requests)} original timed interpolation traces')
if __name__=='__main__':main()
