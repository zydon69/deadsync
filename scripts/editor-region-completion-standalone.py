#!/usr/bin/env python3
"""Run actual Rust completion code against the pinned original source fixture.
No Cargo/shared target. Actual spacing/index/mapping and planner modules; shell
Error/SpacingMode/Section/Grid/event types. The linear Clock host is used only
by four signed-zero viewport cases, never by the7560 rate-completion traces.
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
notes=(a.core/'notes.rs').read_text();kinds=notes[notes.index('pub enum Kind {'):notes.index('#[derive(Clone, Debug, PartialEq, Eq)]')]
src+='mod notes{use std::collections::BTreeMap;#[derive(Clone,Copy,Debug,PartialEq,Eq)]'+kinds+'#[derive(Clone,Debug)]pub struct Note{pub kind:Kind,pub end:Option<u64>}#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Position{pub row:u64,pub lane:u16,pub player:u32}pub struct Grid{pub tracks:BTreeMap<(u32,u16),BTreeMap<u64,Note>>}impl Grid{'+''.join(member(notes,m)for m in ['pub fn get(','pub fn row_heads_from(','pub fn last_head_before(','pub fn visible('])+'}}\n'
src+='mod event_clipboard{use super::notes::Position;#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum WidgetKind{Preview,EndHint}#[derive(Clone,Copy,Debug)]pub struct Widget{pub k:WidgetKind,pub row:f64}impl Widget{pub fn kind(self)->WidgetKind{self.k}}#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum TimingKind{Signature,Tempo,Delay,Scroll,Stop,Warp,Label}#[derive(Clone,Copy,Debug)]pub enum Event{Note(Position),Timing{row:u64,kind:TimingKind},SongWidget(Widget)}impl Event{pub fn chart_position(self)->f64{match self{Self::Note(p)=>p.row as f64,Self::Timing{row,..}=>row as f64,Self::SongWidget(w)=>w.row}}}}\n'
src+='mod playback{use super::Error;pub struct Clock;impl Clock{pub fn event_phase_time(&self,row:u64,_:u8)->Result<f64,Error>{Ok(row as f64/96.0)}}}\n'
src+='mod region_selection{pub mod inventory{use crate::event_clipboard::Event;#[derive(Clone,Copy)]pub struct Entry{pub event:Event,pub phase:u8,pub chart_time:f64}}'+inc('admission',a.core/'region_selection/admission.rs')+('pub mod visible_events {\n'+(a.work_core/'region_selection/visible_events.rs').read_text()+'''
pub fn oracle_trace(index:&Index,mut section:usize,mut anchor:Anchor,settings:Settings,input:Region,max:i32,reached:bool)->Result<RegionCompletion,Error>{let mut c=RegionCompletion::new(input)?;let mut h=coefficients(index,section,settings)?;c.update(h,index,section,anchor,(section+1<index.sections().len()).then_some(section+1),settings)?;finish_region(&mut c,index,&mut section,&mut anchor,&mut h,settings,max,reached)?;Ok(c)}
}''')+'}\n'
src+='use region_selection::{admission::{Admission,Kind},visible_events::{self,Viewport},inventory::Entry};use event_clipboard::{Event,TimingKind};use notes::{Grid,Note,Position};use std::collections::BTreeMap;use generation::editor_timing::SpacingSection;use rate_mapping::RateMapping;use spacing::{Anchor,Speeds};use spacing_search::Settings;use zoom::SpacingMode;fn main(){\n'

cases=json.loads(a.fixture.read_text())
def f(x):return repr(float(x))
src+='for (spacing,reverse,initial,start,current,max,reached,fallback,processed,extra,start_y,time,row)in['+','.join('('+','.join([str(c['spacing']),str(c['reverse']).lower(),str(c['initial']),f(c['start']),f(c['current']),str(c['max']),str(c['reached']).lower(),str(c['fallback']).lower(),str(c['processed'])+'usize',str(c['extra'])+'usize',f(c['startY']),f(c['time']),f(c['row'])])+')'for c in cases)+']{'
src+='let index=spacing_index::Index::new(vec!['+','.join('SpacingSection{order:0,mapping:RateMapping::new('+f(r)+','+f(.75 if r==96 else r/96)+',0.0,0.0,'+f(1/192 if r==48 else 1/96)+').unwrap(),tempo:'+f(240 if r==48 else 120)+',scroll:'+f(-1 if r==96 else 1)+'}'for r in [0,48,96])+']).unwrap();let m=index.sections()[initial].mapping;'
src+='let c=visible_events::oracle_trace(&index,initial,Anchor{y:200.0,time:m.time(),row:m.row()},Settings{mode:match spacing{0=>SpacingMode::ConstantTime,1=>SpacingMode::ConstantRow,_=>SpacingMode::Variable},reverse,speeds:Speeds::default(),interpolated_scroll:1.0,zoom:1.0},visible_events::Region{start_time:start/96.0,start_row:start,current_y:current},max,reached).unwrap();let got=c.current().unwrap().as_ref().unwrap();assert_eq!((c.used_fallback(),c.processed_rates(),c.extra_steps(),c.start_y().unwrap(),got.0,got.1),(fallback,processed,extra,start_y,time,row),"spacing{spacing} reverse{reverse} initial{initial} start{start} current{current} max{max} reached{reached}");}println!("7560 original region-completion cases matched");}\n'
# Also exercise the actual viewport planner at signed-zero scroll. Only the
# Fumen search sentinel crosses nonfinite space; actual admitted rows stay finite.
zero_cases=json.loads(a.fixture.with_name('collapsed-viewport-source.json').read_text())
src=src.rsplit('println!("7560 original region-completion cases matched");}\n',1)[0]
for c in zero_cases:
 zero='-0.0'if c['negativeZero']else'0.0';reverse=str(c['reverse']).lower()
 src+='{let mut tracks=BTreeMap::new();let mut lane=BTreeMap::new();for row in[12,96]{lane.insert(row,Note{kind:notes::Kind::Tap,end:None});}tracks.insert((0,0),lane);let grid=Grid{tracks};let clock=playback::Clock;let index=spacing_index::Index::new(vec![SpacingSection{order:0,mapping:RateMapping::new(0.0,0.0,0.0,0.0,1.0/96.0).unwrap(),tempo:120.0,scroll:'+zero+'}]).unwrap();let misc=[Entry{event:Event::Timing{row:0,kind:TimingKind::Signature},phase:0,chart_time:0.0},Entry{event:Event::Timing{row:0,kind:TimingKind::Tempo},phase:1,chart_time:0.0}];let plan=visible_events::prepare_with_region(&grid,&clock,&index,&misc,Anchor{row:0.0,time:0.0,y:160.0},Settings{mode:SpacingMode::Variable,reverse:'+reverse+',speeds:Speeds::default(),interpolated_scroll:1.0,zoom:1.0},Viewport{top:0.0,bottom:480.0,half_arrow:32.0,hold_cap:32.0},0,true,Some(visible_events::Region{start_time:0.125,start_row:12.0,current_y:160.0}),0).unwrap();assert_eq!(plan.events().len(),1);let e=plan.events()[0];assert!(e.chart_position().is_finite());let id=match e{Event::Timing{kind:TimingKind::Signature,..}=>0,Event::Note(p)if p.row==96=>3,_=>panic!()};assert_eq!(id,'+str(c['index'])+');assert!(plan.completion().unwrap().current().unwrap().is_err());}\n'
src+='println!("7560 region completion cases and4 signed-zero Fumen search cases matched");}\n'
with tempfile.TemporaryDirectory(prefix='editor-region-completion-rustc-')as d:
 d=Path(d);(d/'main.rs').write_text(src);q=subprocess.run(['rustc','--edition=2024',str(d/'main.rs'),'-o',str(d/'oracle')],capture_output=True,text=True)
 if q.returncode:raise RuntimeError(q.stderr)
 q=subprocess.run([str(d/'oracle')],capture_output=True,text=True);print(q.stdout,end='')
 if q.returncode:raise RuntimeError(q.stderr)
