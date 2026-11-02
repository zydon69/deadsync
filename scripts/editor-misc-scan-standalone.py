#!/usr/bin/env python3
"""Compare actual pure Rust modules to the original full-Misc-scan fixture.
No Cargo or shared artifacts. Host types: Error, SpacingMode, SpacingSection
(source type/spacing_rate text copied unchanged from supplied core tree).
"""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--core',type=Path,required=True);p.add_argument('--scan',type=Path,required=True);p.add_argument('--fixture',type=Path,required=True);a=p.parse_args()
def include(name,path):return '#[path='+json.dumps(str(path.resolve()))+'] mod '+name+';\n'
src='#![allow(dead_code)]\n#[derive(Debug)]pub enum Error{InvalidEdit(String)}\nmod zoom{#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum SpacingMode{ConstantTime,ConstantRow,Variable}}\n'
for n in ['spacing','rate_mapping','spacing_index','spacing_search']:src+=include(n,a.core/(n+'.rs'))
timing=(a.core/'generation/editor_timing.rs').read_text();section=timing[timing.index('pub struct SpacingSection {'):timing.index('pub struct EditorTimeline {')]
src+='mod generation{pub mod editor_timing{#[derive(Clone,Copy)]'+section+'}}\n'
src+=include('misc_scan',a.scan)+include('misc_layout',a.core/'region_selection/misc_layout.rs')
src+='use spacing::{Anchor,Speeds};use spacing_search::Settings;use zoom::SpacingMode;use misc_layout::{Kind,Layout};use misc_scan::{Item,Bounds};use generation::editor_timing::SpacingSection;use rate_mapping::RateMapping;\nfn main(){let kinds=[Kind::Signature,Kind::Stop,Kind::Delay,Kind::Warp,Kind::Tick,Kind::Combo,Kind::Label,Kind::Attack,Kind::Preview,Kind::EndHint,Kind::Pattern,Kind::Tempo,Kind::Scroll,Kind::Speed,Kind::Fake];\n'
def f(n):return repr(float(n))
for num,c in enumerate(json.loads(a.fixture.read_text())):
 events=sorted(c['events'],key=lambda e:(e['row'],e['id']));indices={e['id']:i for i,e in enumerate(events)}
 items=','.join('Item{row:'+f(e['row'])+',time:'+f(e['time'])+',phase:'+str(e['id'])+'}'for e in events)
 src+='let index=spacing_index::Index::new(vec!['+','.join('SpacingSection{order:0,mapping:RateMapping::new('+f(row)+','+f(0.75 if row==96 else row/96)+',0.0,0.0,'+f(1/192 if row==48 else 1/96)+').unwrap(),tempo:'+f(240 if row==48 else 120)+',scroll:'+f(-1 if row==96 else 1)+'}'for row in [0,48,96])+']).unwrap();\n'
 src+='let items=['+items+'];let rates=['+','.join('Some('+str(indices[i])+')'for i in range(3))+'];let ids=['+','.join(str(e['id'])+'u64'for e in events)+'];let ks=['+','.join(str(e['kind'])+'usize'for e in events)+'];let mut layout=Layout::new(-130.0,130.0,100,2.0).unwrap();let mut calls=Vec::new();\n'
 mode=['ConstantTime','ConstantRow','Variable'][c['spacing']];fo=c['focal'];mi=c['min'];reverse=str(c['reverse']).lower()
 src+='let result=misc_scan::scan(&index,Anchor{y:200.0,row:'+f(fo)+',time:'+f(.75 if fo==96 else fo/96)+'},Settings{mode:SpacingMode::'+mode+',reverse:'+reverse+',speeds:Speeds::default(),interpolated_scroll:1.0,zoom:1.0},Bounds{start_time:'+f(mi/96)+',end_time:'+f((mi+48)/96)+',start_row:'+f(mi)+',end_row:'+f(mi+48)+'},&items,&rates,|i,y|{calls.push((ids[i],y));layout.position(ids[i],kinds[ks[i]],'+('items[i].time'if c['spacing']==0 else'items[i].row')+',y,80.0,20.0).unwrap();Ok(layout.maximum_single_row_height(20.0).unwrap())}).unwrap();\n'
 expected=','.join('('+str(x['id'])+'u64,'+f(x['y'])+')'for x in c['calls']);src+='assert_eq!(calls,vec!['+expected+'],"calls case'+str(num)+'");'
 src+='let (lo,hi)=(result.start_y.unwrap(),result.end_y.unwrap());assert_eq!((lo.min(hi),lo.max(hi)),('+f(c['top'])+','+f(c['bottom'])+'),"bounds case'+str(num)+'");\n'
 for e in events:
  if e['positioned']:src+='let r=layout.rectangle('+str(e['id'])+').unwrap();assert_eq!((r.x,r.y,r.width,r.height),('+','.join(f(e[k])for k in ['x','y','w','h'])+'),"rect case'+str(num)+' id'+str(e['id'])+'");\n'
  else:src+='assert!(layout.rectangle('+str(e['id'])+').is_none(),"absent case'+str(num)+'");\n'
src+='println!("216 original full scans: calls, bounds, final rectangles matched");}\n'
with tempfile.TemporaryDirectory(prefix='editor-misc-rustc-')as d:
 d=Path(d);(d/'main.rs').write_text(src);q=subprocess.run(['rustc','--edition=2024',str(d/'main.rs'),'-o',str(d/'oracle')],capture_output=True,text=True)
 if q.returncode:raise RuntimeError(q.stderr)
 q=subprocess.run([str(d/'oracle')],capture_output=True,text=True)
 print(q.stdout,end='')
 if q.returncode:raise RuntimeError(q.stderr)
