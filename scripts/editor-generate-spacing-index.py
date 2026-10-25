#!/usr/bin/env python3
"""Execute original active-rate selection methods on the real Fumen tree."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
ROOT=Path(__file__).resolve().parents[1]
PIN='48e18f71655d0f0d86144c63239331833501fda2'
FUMEN_PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def member(source,pattern):
    match=re.search(pattern,source)
    if match is None:raise SystemExit(f'Missing member: {pattern}')
    start=source.index('{',match.start());depth=1;end=start+1
    while depth:
        if source[end]=='{':depth+=1
        elif source[end]=='}':depth-=1
        end+=1
    return source[match.start():end]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--editor-root',type=Path,required=True);parser.add_argument('--reference-root',type=Path,required=True);parser.add_argument('--dotnet',default='dotnet');args=parser.parse_args()
    if subprocess.check_output(['git','rev-parse','HEAD'],cwd=args.reference_root/'Fumen',text=True).strip()!=FUMEN_PIN:raise SystemExit('Fumen pin mismatch')
    def read(path):return subprocess.check_output(['git','show',f'{PIN}:StepManiaEditor/EditorEvents/Containers/{path}'],cwd=args.editor_root).decode('utf-8-sig')
    tree=read('RateAlteringEventTree.cs');utils=read('EventTreeUtils.cs')
    tree_members=[member(tree,r'\tpublic IReadOnlyRedBlackTree<EditorRateAlteringEvent>\.IReadOnlyRedBlackTreeEnumerator '+name+r'\(') for name in ['FindBest','FindBestByTime','FindBestByPosition']]
    utils_members=[member(utils,r'\tpublic static bool '+name+r'<T>\(') for name in ['EnsureGreatestLessThanTime','EnsureLeastGreaterThanOrEqualToTime','EnsureGreatestLessThanPosition','EnsureLeastGreaterThanOrEqualToPosition','UnsetAndReturnIfWasValid']]
    project=ROOT/'scripts/editor-spacing-index-oracle';(project/'obj').mkdir(exist_ok=True)
    (project/'obj/Selection.cs').write_text('using Fumen;\nusing static StepManiaEditor.EventTreeUtils;\nnamespace StepManiaEditor;\ninternal sealed partial class RateAlteringEventTree {\n'+'\n'.join(tree_members)+'\n}\n')
    (project/'obj/Utils.cs').write_text('using Fumen;\nnamespace StepManiaEditor;\ninternal static class EventTreeUtils {\n'+'\n'.join(utils_members)+'\n}\n')
    env=dict(os.environ,DOTNET_NOLOGO='1',DOTNET_CLI_TELEMETRY_OPTOUT='1')
    subprocess.run([args.dotnet,'build',str(project/'IndexOracle.csproj'),'-c','Release',f'-p:ReferenceRoot={args.reference_root.resolve()}','--ignore-failed-sources'],check=True,env=env)
    fixtures=ROOT/'crates/deadsync-editor/tests/fixtures/grooveauthor';requests=sorted(fixtures.glob('spacing-index-*.request.json'))
    if not requests:raise SystemExit('No spacing index cases')
    for path in requests:
        output=subprocess.check_output([args.dotnet,str(project/'bin/Release/net10.0/IndexOracle.dll'),str(path)],env=env)
        path.with_name(path.name.replace('.request.json','.expected.json')).write_bytes(output)
    manifest={'GrooveAuthor':PIN,'Fumen':FUMEN_PIN,'FumenAssemblySHA256':hashlib.sha256((args.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()).hexdigest(),'TreeMembersSHA256':hashlib.sha256('\n'.join(tree_members).encode()).hexdigest(),'UtilsMembersSHA256':hashlib.sha256('\n'.join(utils_members).encode()).hexdigest(),'Cases':[p.stem for p in requests],'Scope':'Original FindBest/Ensure methods, actual Fumen tree; scalar event comparator preserves supplied chart order and original row/time-only search comparisons'}
    (fixtures/'spacing-index-reference.json').write_text(json.dumps(manifest,indent=2)+'\n');print(f'Generated {len(requests)} original tree selection traces')
if __name__=='__main__':main()
