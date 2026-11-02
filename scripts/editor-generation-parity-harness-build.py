#!/usr/bin/env python3
"""Rebuild the existing original-search harness in an isolated directory.
Regenerates every adapter from pinned original source; requires no ignored obj
or pre-existing binary. Never writes the repository or product fixtures.
"""
import argparse,hashlib,importlib.util,json,shutil,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',required=True,type=Path);p.add_argument('--editor',required=True,type=Path);p.add_argument('--library',required=True,type=Path);p.add_argument('--dotnet',required=True,type=Path);p.add_argument('--build-dir',required=True,type=Path);a=p.parse_args();repo=a.repo.resolve();build=a.build_dir.resolve()
assert not build.is_relative_to(repo),'Build directory must be outside repository'
for root,pin in [(a.editor,'48e18f71655d0f0d86144c63239331833501fda2'),(a.library,'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'),(a.library/'Fumen','0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89')]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
project=build/'scripts/editor-oracle';project.mkdir(parents=True,exist_ok=True)
for path in (repo/'scripts/editor-oracle').iterdir():
 if path.is_file():shutil.copy2(path,project/path.name)
spec=importlib.util.spec_from_file_location('original_generator',repo/'scripts/editor-generate-fixtures.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.ROOT=build
members={}
for name in ['context','timing','metadata','profiles','tempo','pause','region','source_timing','signature','scroll','tick','combo','speed','label','pattern_navigation','music_rate']:
 members[name]=getattr(module,'prepare_'+name+'_reference')(a.editor.resolve())
subprocess.run([str(a.dotnet),'build',str(project/'EditorOracle.csproj'),'-c','Release','--nologo','-v','quiet','-p:ReferenceRoot='+str(a.library.resolve())],check=True)
assembly=project/'bin/Release/net10.0/EditorOracle.dll';sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest();sources={str(path.relative_to(project)):sha(path)for path in [project/'Program.cs',*(project/'obj').glob('*Reference.cs')]}
(project/'generation-build-provenance.json').write_text(json.dumps(dict(original_members=members,sources=sources,assembly_sha256=sha(assembly),scope='Original search methods run against original libraries; adapters regenerated from pinned original editor. No Rust execution.'),indent=2)+'\n');print(assembly)
