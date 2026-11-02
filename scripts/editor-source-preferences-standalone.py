#!/usr/bin/env python3
"""Type-check/execute actual pure source-import parser helpers, isolated rustc host."""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['module','core','deps','fixture']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();s=a.module.read_text();preamble=s[s.index('pub const MAX_SOURCE_BYTES'):s.index('#[derive(Clone)]\npub struct Snapshot')];body=s[s.index('fn invalid('):s.index('impl Draft{\n pub fn prepare')]
body=body.replace('include_str!("source_keys.json")','include_str!('+json.dumps(str(a.module.with_name('source_keys.json').resolve()))+')').replace('include_str!("../key_binding_preferences/source_defaults.json")','include_str!('+json.dumps(str((a.core/'key_binding_preferences/source_defaults.json').resolve()))+')')
code='''#![allow(dead_code,unused_imports)]
use serde_json::{Map,Value};use std::collections::BTreeSet;
#[derive(Debug)]enum Error{InvalidFormat(String)}
mod source_profile{#[derive(Clone,Debug,Default,PartialEq)]pub struct Report{pub rejected:Vec<String>,pub unavailable_commands:Vec<String>,pub neutralized_native_extensions:Vec<String>,pub retained_modifiers:Vec<String>,pub retained_unknown:Vec<String>}}
mod generation{#[path=JSONC]pub mod jsonc;}
'''.replace('JSONC',json.dumps(str((a.core/'generation/jsonc.rs').resolve())))+preamble+body
shared=(Path(__file__).resolve().parent.parent/'patches/editor-source-post-load.rs').read_text().replace('use crate::error::Error;','use crate::Error;').replace('include_str!(\"source_keys.json\")','include_str!('+json.dumps(str((Path(__file__).resolve().parent.parent/'patches/editor-source-post-load-keys.json').resolve()))+')')
code+='\nmod key_binding_preferences {pub mod source_post_load {\n'+shared+'\n}}\n'
code+='''fn main(){let fixture:Value=serde_json::from_str(include_str!(FIXTURE)).unwrap();let mut checked=0;for case in fixture["cases"].as_array().unwrap(){let name=case["name"].as_str().unwrap();if name.starts_with("log-")||name.starts_with("ftue-"){continue;}let raw=case["raw"].as_str().unwrap();let normalized=match source_root(raw.as_bytes()){Ok(value)=>value,Err(_)=>{assert_eq!(name,"unknown-depth-65");checked+=1;continue;}};let root:std::collections::BTreeMap<String,&serde_json::value::RawValue>=serde_json::from_slice(&normalized).unwrap();let mut schema=Map::new();for(_,native)in GROUPS{schema.insert(native.into(),serde_json::json!({"Probe":0}));}schema.insert("Zoom".into(),serde_json::json!({}));let mut report=Report::default();let source=match decode_source(&root,&schema,&mut report){Ok(value)=>value,Err(_)=>{assert!(name=="known-huge-integer"||name=="key-undefined-out-of-array","unexpected failure {name}");checked+=1;continue;}};if let Some(player)=source.get("Player"){assert_eq!(player,&case["expected"]["value"]["Player"],"player {name}");}if let Some(mode)=source.get("NoteEntryMode"){let(value,_,_)=enum_value(mode,EnumSpec{field:"NoteEntryMode",names:&["Normal","AdvanceBySnap"],invalid_string:0});assert_eq!(value,case["expected"]["value"]["NoteEntryMode"],"mode {name}");}for group in["PreferencesAudio","PreferencesWaveForm","PreferencesMultiplayer"]{if let Some(value)=source.get(group){assert_eq!(value["Probe"],case["expected"]["value"][group]["Probe"],"group {group}/{name}");}}if let Some(value)=source.get("PreferencesKeyBinds").and_then(|v|v.get("Save")){assert_eq!(value,&case["expected"]["value"]["PreferencesKeyBinds"]["Save"],"key PostLoad {name}");}checked+=1;}println!("{checked} actual parser/PostLoad cases match scoped original root oracle");}
'''.replace('FIXTURE',json.dumps(str(a.fixture.resolve())))
# serde and serde_json may have several feature-built instances. Try available
# immutable artifacts; all output remains in an isolated temporary directory.
jsons=sorted(a.deps.glob('libserde_json-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True);serdes=sorted(a.deps.glob('libserde-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True)
def raw_enabled(path):
 fingerprint=a.deps.parent/'.fingerprint'/('serde_json-'+path.stem.split('-')[-1])/'lib-serde_json.json'
 return fingerprint.exists() and 'raw_value' in json.loads(fingerprint.read_text()).get('features','')
jsons=[path for path in jsons if raw_enabled(path)]
def core_dependency(path,crate):
 fingerprint=a.deps.parent/'.fingerprint'/(crate+'-'+path.stem.split('-')[-1])/('lib-'+crate+'.json')
 if not fingerprint.exists():return None
 return next((dep[-1] for dep in json.loads(fingerprint.read_text()).get('deps',[]) if dep[1]=='serde_core'),None)
if jsons:
 required=core_dependency(jsons[0],'serde_json');serdes=[path for path in serdes if core_dependency(path,'serde')==required]

with tempfile.TemporaryDirectory(prefix='source-preferences-rustc-')as tmp:
 d=Path(tmp);(d/'main.rs').write_text(code);errors=[]
 for js in jsons[:1]:
  for serde in serdes[:1]:
   q=subprocess.run(['rustc','--edition=2024',str(d/'main.rs'),'--extern','serde='+str(serde.resolve()),'--extern','serde_json='+str(js.resolve()),'-L','dependency='+str(a.deps.resolve()),'-o',str(d/'oracle')],capture_output=True,text=True)
   if q.returncode==0:
    q=subprocess.run([str(d/'oracle')],capture_output=True,text=True);print(q.stdout,end='')
    if q.returncode:raise RuntimeError(q.stderr)
    print('Host scope: schema Probe groups/linear metadata, no File/history/native effects; real parser, enum/key helpers, JSONC and compiled serde.');raise SystemExit(0)
   errors.append(q.stderr)
   if 'error[E' in q.stderr and 'trait bound' not in q.stderr and 'multiple different versions' not in q.stderr:
    # A normal type error is not repaired by selecting another dependency hash.
    raise RuntimeError(q.stderr)
 raise RuntimeError(errors[-1]if errors else'No prebuilt serde artifacts found')
