#!/usr/bin/env python3
"""Execute unchanged original spacing helper with explicit scalar adapters."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
PIN = "48e18f71655d0f0d86144c63239331833501fda2"
ROOT = Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--editor-root',type=Path,required=True)
    parser.add_argument('--dotnet',default='dotnet')
    args=parser.parse_args()
    source=subprocess.check_output(['git','show',f'{PIN}:StepManiaEditor/EventSpacingHelper.cs'],cwd=args.editor_root)
    project=ROOT/'scripts/editor-spacing-oracle'
    (project/'obj').mkdir(exist_ok=True)
    (project/'obj/EventSpacingHelper.cs').write_bytes(source)
    env=dict(os.environ,DOTNET_CLI_TELEMETRY_OPTOUT='1',DOTNET_NOLOGO='1')
    subprocess.run([args.dotnet,'build',str(project/'SpacingOracle.csproj'),'-c','Release','--ignore-failed-sources'],check=True,env=env)
    destination=ROOT/'crates/deadsync-editor/tests/fixtures/grooveauthor'
    requests=sorted(destination.glob('spacing-*.request.json'))
    if not requests: raise SystemExit('No spacing requests found')
    for path in requests:
        output=subprocess.check_output([args.dotnet,str(project/'bin/Release/net10.0/SpacingOracle.dll'),str(path)],env=env)
        path.with_name(path.name.replace('.request.json','.expected.json')).write_bytes(output)
    manifest={'GrooveAuthor':PIN,'EventSpacingHelperSHA256':hashlib.sha256(source).hexdigest(),'Cases':[p.stem for p in requests],'AdapterScope':'Scalar preferences, event/region accessors and explicit linear rate conversion callbacks only; original helper unchanged'}
    (destination/'spacing-reference.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f'Generated {len(requests)} original spacing traces')
if __name__=='__main__':main()
