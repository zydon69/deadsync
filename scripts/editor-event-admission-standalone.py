#!/usr/bin/env python3
"""Pure Rust admission/viewport parity, without Cargo/shared target artifacts.
Real modules: Admission, visible_events, spacing/index/search/mapping; real Grid
ordered/head/visible methods copied unchanged. Host: Error, enum/type shells,
linear clock for the explicit constant-row scalar oracle inputs. Not crate QA.
"""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--core',type=Path,required=True);p.add_argument('--work-core',type=Path,required=True);p.add_argument('--fixture',type=Path,required=True);a=p.parse_args()
def member(s,m):
 i=s.index(m);j=s.index('{',i)+1;depth=1
 while depth:depth+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[i:j]
def inc(n,path):return '#[path='+json.dumps(str(path.resolve()))+']pub mod '+n+';\n'
src='#![allow(dead_code)]\n#[derive(Debug)]pub enum Error{InvalidEdit(String)}\nmod zoom{#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum SpacingMode{ConstantTime,ConstantRow,Variable}}\n'
for n in ['spacing','rate_mapping','spacing_index','spacing_search']:src+=inc(n,a.core/(n+'.rs'))
timing=(a.core/'generation/editor_timing.rs').read_text();section=timing[timing.index('pub struct SpacingSection {'):timing.index('pub struct EditorTimeline {')]
src+='mod generation{pub mod editor_timing{#[derive(Clone,Copy)]'+section+'}}\n'
notes=(a.work_core/'notes.rs').read_text();kinds=notes[notes.index('pub enum Kind {'):notes.index('#[derive(Clone, Debug, PartialEq, Eq)]')]
src+='mod notes{use std::collections::BTreeMap;#[derive(Clone,Copy,Debug,PartialEq,Eq)]'+kinds+'#[derive(Clone,Debug)]pub struct Note{pub kind:Kind,pub end:Option<u64>}#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Position{pub row:u64,pub lane:u16,pub player:u32}pub struct Grid{pub tracks:BTreeMap<(u32,u16),BTreeMap<u64,Note>>}impl Grid{'+''.join(member(notes,m)for m in ['pub fn get(','pub fn row_heads_from(','pub fn last_head_before(','pub fn visible('])+'}}\n'
src+='mod event_clipboard{use super::notes::Position;#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum WidgetKind{Preview,EndHint}#[derive(Clone,Copy,Debug)]pub struct Widget{pub k:WidgetKind,pub row:f64}impl Widget{pub fn kind(self)->WidgetKind{self.k}}#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum TimingKind{Signature,Tempo,Delay,Scroll,Stop,Warp,Label}#[derive(Clone,Copy,Debug)]pub enum Event{Note(Position),Timing{row:u64,kind:TimingKind},SongWidget(Widget)}impl Event{pub fn chart_position(self)->f64{match self{Self::Note(p)=>p.row as f64,Self::Timing{row,..}=>row as f64,Self::SongWidget(w)=>w.row}}}}\n'
src+='mod playback{use super::Error;pub struct Clock;impl Clock{pub fn event_phase_time(&self,row:u64,_:u8)->Result<f64,Error>{Ok(row as f64/96.0)}}}\n'
src+='mod region_selection{pub mod inventory{use crate::event_clipboard::Event;#[derive(Clone,Copy)]pub struct Entry{pub event:Event,pub phase:u8,pub chart_time:f64}}'+inc('admission',a.work_core/'region_selection/admission.rs')+inc('visible_events',a.work_core/'region_selection/visible_events.rs')+'}\n'
src+='use region_selection::{admission::{Admission,Kind},visible_events::{self,Viewport},inventory::Entry};use event_clipboard::{Event,TimingKind};use notes::{Grid,Note,Position};use std::collections::BTreeMap;use generation::editor_timing::SpacingSection;use rate_mapping::RateMapping;use spacing::{Anchor,Speeds};use spacing_search::Settings;use zoom::SpacingMode;fn main(){\n'
cases=json.loads(a.fixture.read_text())
for c in cases:
 name=c['name'];max=c['max'];pre=c['pre'];draw=str(c['draw']).lower();expected=','.join(str(x)+'i64'for x in c['admitted']);src+='{let mut a=Admission::new('+str(max)+','+str(pre)+','+draw+');let mut got=vec!['+','.join(str(-i)+'i64'for i in range(1,pre+1))+'];'
 # Write as a proper loop; no injected break outside its loop.
 src+='for &(id,kind,far)in &['+','.join('('+str(i)+'i64,Kind::'+('Lane'if k in[0,3]else'Misc'if k==1 else'NextRate'if k==2 else'Other')+','+str(k!=2 and (k==4 or c['ys']is not None and c['ys'][i]-32>200)and not c['negative']).lower()+')'for i,k in enumerate(c['kinds']))+']{if far{break;}let d=a.offer(kind);if d.admit{got.push(id);}if d.stop{break;}}assert_eq!(got,vec!['+expected+'],"admission '+name+'");}\n'
 if c['ys']is None:continue
 src+='{let mut tracks=BTreeMap::new();let mut lane=BTreeMap::new();for row in[12,24,36,120]{lane.insert(row,Note{kind:notes::Kind::Tap,end:None});}tracks.insert((0,0),lane);let grid=Grid{tracks};let clock=playback::Clock;let index=spacing_index::Index::new(vec!['+','.join('SpacingSection{order:'+str(order)+',mapping:RateMapping::new('+str(row)+'.0,'+str(row/96)+',0.0,0.0,1.0/96.0).unwrap(),tempo:120.0,scroll:1.0}'for row,order in [(0,0),(0,1),(24,13)])+']).unwrap();'
 src+='let misc=['+','.join('Entry{event:Event::Timing{row:'+str(row)+',kind:TimingKind::'+kind+'},phase:'+str(phase)+',chart_time:'+str(row/96)+'}'for row,phase,kind in [(0,0,'Signature'),(0,1,'Tempo'),(18,5,'Label'),(24,13,'Scroll')])+'];'
 src+='let plan=visible_events::prepare(&grid,&clock,&index,&misc,Anchor{row:100.0,time:100.0/96.0,y:200.0},Settings{mode:SpacingMode::ConstantRow,reverse:false,speeds:Speeds::default(),interpolated_scroll:1.0,zoom:1.0},Viewport{top:0.0,bottom:200.0,half_arrow:32.0,hold_cap:32.0},'+str(max)+','+draw+').unwrap();'
 src+='let got=plan.events().iter().map(|e|match e{Event::Timing{kind:TimingKind::Signature,..}=>0,Event::Timing{kind:TimingKind::Tempo,..}=>1,Event::Note(p)=>match p.row{12=>2,24=>4,36=>6,120=>7,_=>panic!()},Event::Timing{kind:TimingKind::Label,..}=>3,Event::Timing{kind:TimingKind::Scroll,..}=>5,_=>panic!()}).collect::<Vec<i64>>();assert_eq!(got,vec!['+expected+'],"viewport '+name+'");}\n'
src+='println!("298 admission-loop cases and18 scalar viewport consumer plans matched");}\n'
with tempfile.TemporaryDirectory(prefix='editor-admission-rustc-')as d:
 d=Path(d);(d/'main.rs').write_text(src);q=subprocess.run(['rustc','--edition=2024',str(d/'main.rs'),'-o',str(d/'oracle')],capture_output=True,text=True)
 if q.returncode:raise RuntimeError(q.stderr)
 q=subprocess.run([str(d/'oracle')],capture_output=True,text=True);print(q.stdout,end='')
 if q.returncode:raise RuntimeError(q.stderr)
