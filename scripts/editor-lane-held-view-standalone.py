#!/usr/bin/env python3
"""Adapt admission standalone host to real sparse held reader; no Cargo."""
import argparse,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--core',type=Path,required=True);p.add_argument('--work-core',type=Path,required=True);p.add_argument('--fixture',type=Path,required=True);a=p.parse_args()
script=(Path(__file__).with_name('editor-event-admission-standalone.py')).read_text()
script=script.replace("a.work_core/'region_selection/admission.rs'","a.core/'region_selection/admission.rs'")
script=script.replace("'pub fn last_head_before(','pub fn visible('","'pub fn last_head_before(','pub(crate) fn last_head_before_excluding(','pub fn visible('")
script=script.replace('PartialEq,Eq)]pub struct Position','PartialEq,Eq,PartialOrd,Ord)]pub struct Position').replace('#[derive(Clone,Debug)]pub struct Note{pub kind:Kind,pub end:Option<u64>}','#[derive(Clone,Debug,PartialEq,Eq)]pub struct Note{pub kind:Kind,pub end:Option<u64>,pub attributes:String,pub tail_attributes:String}')
script=script.replace('kind:notes::Kind::Tap,end:None','kind:notes::Kind::Tap,end:None,attributes:Default::default(),tail_attributes:Default::default()')
lane=(a.work_core/'lane_entry.rs').read_text();head=lane[lane.index('/// One actual event-tree identity.'):]
fresh=lane[lane.index('fn fresh('):lane.index('fn resolve(')]
prefix='mod lane_entry{use crate::notes::{Grid,Kind,Note,Position};use std::collections::BTreeSet;#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Draft{pub position:Position,pub kind:Kind,pub end:Option<u64>}pub struct Inputs{pub deleted:Vec<Position>,pub drafts:Vec<Draft>}impl Inputs{pub fn deleted_heads(&self)->impl Iterator<Item=Position>+\'_ {self.deleted.iter().copied()}pub fn drafts(&self)->impl Iterator<Item=Draft>+\'_ {self.drafts.iter().copied()}}'
insert="src+="+repr(prefix+fresh+head+'}\n')+'\n'
script=script.replace("src+='mod event_clipboard",insert+"src+='mod event_clipboard",1)
# Explicit fixture tests held identity merge and strict edited hold endpoints;
# chart/Inputs host construction is intentional, not original lane transaction QA.
extra=r'''{let p=Position{row:48,lane:0,player:0};let mut track=BTreeMap::new();track.insert(48,Note{kind:notes::Kind::Tap,end:None,attributes:Default::default(),tail_attributes:Default::default()});let grid=Grid{tracks:BTreeMap::from([((0,0),track)])};let input=lane_entry::Inputs{deleted:vec![],drafts:vec![lane_entry::Draft{position:p,kind:notes::Kind::Tap,end:None}]};let held=lane_entry::Held::new(&grid,Some(&input));let h=held.heads_from(0).collect::<Vec<_>>();assert_eq!(h.len(),2);assert!(!h[0].edited);assert!(h[1].edited);assert_eq!(held.last_head_before(49).unwrap(),h[1]);let input=lane_entry::Inputs{deleted:vec![p],drafts:vec![lane_entry::Draft{position:Position{row:24,..p},kind:notes::Kind::Hold,end:Some(120)}]};let held=lane_entry::Held::new(&grid,Some(&input));assert_eq!(held.heads_from(0).count(),1);assert_eq!(held.crossing(24.0,24).len(),0);assert_eq!(held.crossing(25.0,25).len(),1);assert_eq!(held.crossing(120.0,120).len(),0);}
'''
script=script.replace("src+='println!",'src+='+repr(extra)+'\n'+"src+='println!",1)
with tempfile.TemporaryDirectory(prefix='held-view-harness-') as tmp:
 q=Path(tmp)/'h.py';q.write_text(script);subprocess.run(['python3',str(q),'--core',str(a.core),'--work-core',str(a.work_core),'--fixture',str(a.fixture)],check=True)
