#!/usr/bin/env python3
"""Execute real leaf-patch tests via isolated rustc, no shared Cargo target write."""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--module',type=Path,required=True);p.add_argument('--deps',type=Path,required=True);a=p.parse_args()
s=a.module.read_text();body=s[s.index('#[derive(Clone,Debug)]\nstruct Leaf'):s.index('/// One history entry')];tests=s[s.index('#[cfg(test)]'):]
code='''use serde_json::{Map,Value};use std::sync::Arc;
#[derive(Debug)]enum Error{InvalidFormat(String)}
fn invalid(s:impl Into<String>)->Error{Error::InvalidFormat(s.into())}
'''+body+tests
for js in sorted(a.deps.glob('libserde_json-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True):
 fp=a.deps.parent/'.fingerprint'/('serde_json-'+js.stem.split('-')[-1])/'lib-serde_json.json'
 if fp.exists() and 'raw_value' in json.loads(fp.read_text()).get('features',''):break
else:raise RuntimeError('No immutable raw_value serde_json artifact')
with tempfile.TemporaryDirectory(prefix='preference-leaf-tests-')as tmp:
 d=Path(tmp);(d/'tests.rs').write_text(code)
 subprocess.run(['rustc','--edition=2024','--test',str(d/'tests.rs'),'--extern','serde_json='+str(js.resolve()),'-L','dependency='+str(a.deps.resolve()),'-o',str(d/'tests')],check=True)
 subprocess.run([str(d/'tests')],check=True)
print('Scope: actual leaf-merge/CAS implementation and tests; no File IO, Session history or native effects.')
