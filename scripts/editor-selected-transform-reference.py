#!/usr/bin/env python3
"""Execute original spatial/owner actions; preserve their distinct insertion contracts."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
GA='48e18f71655d0f0d86144c63239331833501fda2';LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';FUMEN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
for folder,pin in [(a.editor_root,GA),(a.reference_root,LIB),(a.reference_root/'Fumen',FUMEN)]:assert subprocess.check_output(['git','-C',str(folder),'rev-parse','HEAD'],text=True).strip()==pin
sources={}
def source(name):
 b=subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/{name}']);sources[name]=b;return b.decode('utf-8-sig')
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);end=brace+1;depth=1
 while depth:
  if text[end]=='{':depth+=1
  if text[end]=='}':depth-=1
  end+=1
 return text[start:end]
repo=Path(__file__).resolve().parent.parent;project=repo/'scripts/editor-selected-transform-oracle'
chart=source('EditorChart.cs');event=source('EditorEvents/EditorEvent.cs');pattern=source('EditorEvents/EditorPatternEvent.cs')
pieces={'ORIGINAL_REPOSITIONABLE':member(event,'public bool CanBeRepositioned()'),'ORIGINAL_DEFINITION':member(pattern,'public class Definition'),'ORIGINAL_PATTERN_CLONE':member(pattern,'public override EditorEvent Clone('),'ORIGINAL_IS_MULTIPLAYER':member(chart,'public bool IsMultiPlayer()'),'ORIGINAL_CAN_EXIST':member(chart,'public bool CanEventExistAtRow('),'ORIGINAL_FORCE_ADD':member(chart,'private void ForceAddEventInternal(')}
base=(repo/'scripts/editor-event-shift-oracle/Adapter.cs.template').read_text()
for name,body in pieces.items():base=base.replace('// '+name,body)
base=base.replace('internal sealed class Editor(int player) {','internal sealed class Editor(int player) {public StepManiaLibrary.PadData Pad;public StepManiaLibrary.PadData GetPadData(ChartType type)=>Pad;')
base=base.replace('public int GetLane() =>','public void SetLane(int lane){if(Value is LaneNote note)note.Lane=lane;}\npublic int GetLane() =>',1)
base=base.replace('public readonly int NumInputs;','public readonly ChartType ChartType; public readonly int NumInputs;',1)
base=base.replace("public EditorChart(string layout, int players) {", "public EditorChart(string layout, int players) {ChartType=Enum.Parse<ChartType>(layout.Replace('-', '_'));")
force=member(chart,'public (List<EditorEvent>, List<EditorEvent>) ForceAddEvents(')
base=base.replace('// ORIGINAL_FORCE_ADD', '') # placeholder was already expanded
anchor='    public void ForceAddEventsComplete(List<EditorEvent> events) { }'
assert anchor in base;base=base.replace(anchor,'    private static void Assert(bool value){} public bool CanBeEdited()=>true;\n'+force+'\n'+anchor,1)
base=base.replace('public virtual object Project() => new { Kind = Value.GetType().Name,','public virtual object Project() => new { Kind = Value is LaneNote ? Value.SourceType switch {"1"=>"Tap","M"=>"Mine","F"=>"Fake","L"=>"Lift","2"=>"Hold","4"=>"Roll",_=>"Note"} : Value.GetType().Name,',1)
base=base.replace('"Hold" => new EditorHoldNoteEvent', '"Roll" => new EditorHoldNoteEvent(row, lane, player, item.GetProperty("End").GetInt32(), true, chart),\n            "Mine" or "Fake" or "Lift" => new EditorTapEvent(new LaneTapNote {IntegerPosition=row,Lane=lane,Player=player,SourceType=item.GetProperty("Kind").GetString() switch {"Mine"=>"M","Fake"=>"F",_=>"L"}},chart),\n            "Hold" => new EditorHoldNoteEvent',1)
# A normal owner action adds duplicate addresses to the host event list; this is
# intentional and must remain observable in action output and real writer readback.
song='''public Song Song(string layout,int players){var song=new Song{SourceType=FileFormatType.SSC,Title="Transform",Extras=new Extras()};var c=new Chart{Type=layout,NumInputs=NumInputs,NumPlayers=players,DifficultyType="Hard",DifficultyRating=9,Extras=new Extras()};c.Layers.Add(new Layer{Events=EditorEvents.Values.SelectMany(e=>e.FileEvents()).OrderBy(e=>e,new SMEventComparer(StepManiaLibrary.EventOrder.Order)).ToList()});song.Charts.Add(c);return song;}'''
base=base.replace('public object[] Project() => EditorEvents.Values.Select(value => value.Project()).ToArray();','public object[] Project() => EditorEvents.Values.Select(value => value.Project()).ToArray();\n'+song,1)
base=base.replace('public virtual object Project() =>','public IEnumerable<Event> FileEvents(){yield return Value;if(Value is LaneHoldStartNote n)yield return new LaneHoldEndNote{IntegerPosition=End,Lane=n.Lane,Player=n.Player,SourceType="3"};}\npublic virtual object Project() =>',1)
editor_source=source('Editor.cs');shift_rows_method=member(editor_source,'public int GetShiftNotesRows()')
base+='\ninternal sealed class ShiftRowsReference {public readonly SnapManagerStub SnapManager=new();'+shift_rows_method+'} internal sealed class SnapManagerStub {public int Rows;public int GetCurrentRows()=>Rows;}\n'
action_names=['ActionTransformSelectionLanes','ActionMirrorSelection','ActionFlipSelection','ActionMirrorAndFlipSelection','ActionShiftSelectionLane','ActionShiftSelectionRow','ActionChangeNotePlayer','ActionSwapNotePlayer','ForceAddSideEffect'];actions={name:source('EditorActions/'+name+'.cs')for name in action_names}
def e(kind,row,lane=0,player=0,selected=True,end=None):
 v=dict(Kind=kind,Row=row,Lane=lane,Player=player,Selected=selected)
 if end is not None:v['End']=end
 return v
cases=[]
def add(name,op,events,layout='dance-single',players=1,**extra):cases.append(dict(Name=name,Operation=op,Layout=layout,Players=players,Events=[dict(Kind='Tempo',Row=0,Bpm=120,Selected=False)]+events,**extra))
for op in ['Mirror','Flip','MirrorAndFlip','Left','Right','LeftWrap','RightWrap']:
 add(op+'-exact-peers',op,[e('Tap',48,0),e('Mine',48,1,selected=False),e('Tap',96,3,selected=False)])
add('mirror-trims-unselected-hold','Mirror',[e('Tap',48,0),e('Hold',0,3,selected=False,end=96)])
add('mirror-hold-removes-unselected-heads','Mirror',[e('Hold',48,0,end=192),e('Tap',96,3,selected=False),e('Mine',144,3,selected=False)])
add('flip-unmapped-threepanel','Flip',[e('Tap',48,1)],'dance-threepanel')
add('left-boundary-deletes-selected','Left',[e('Tap',48,0),e('Tap',48,2,selected=False)])
add('right-boundary-deletes-selected','Right',[e('Roll',48,3,end=96),e('Tap',48,0,selected=False)])
add('routine-cross-owner-conflict','Mirror',[e('Tap',48,0,0),e('Hold',0,7,1,False,96)],'dance-routine',3)
for rows in [-48,48,0]:add('rows-'+str(rows),'Rows',[e('Tap',0,0),e('Tap',96,1),e('Tap',144,1,selected=False)],Rows=rows)
add('owner-assign-exact-third-peer','Assign',[e('Hold',48,0,0,end=96),e('Tap',48,1,1,False),e('Tap',96,2,2,False)],'dance-routine',3,PlayerA=1)
add('owner-swap-three-player','SwapPlayers',[e('Tap',48,0,0),e('Hold',96,1,1,end=192),e('Tap',144,2,2)],'dance-routine',3,PlayerA=0,PlayerB=1)
add('owner-assignment-duplicate-address','Assign',[e('Tap',48,0,0),e('Mine',48,0,1,False)],'dance-routine',2,PlayerA=1)
add('owner-assignment-selected-duplicate-address','Assign',[e('Tap',48,0,0),e('Mine',48,0,1)],'dance-routine',2,PlayerA=1)
add('owner-assignment-overlap-hold','Assign',[e('Tap',48,0,0),e('Hold',0,0,1,False,96)],'dance-routine',2,PlayerA=1)
add('owner-swap-identical','SwapPlayers',[e('Tap',48,0,0),e('Tap',48,1,1)],'dance-routine',2,PlayerA=1,PlayerB=1)
request={'Cases':cases};sha=lambda b:hashlib.sha256(b).hexdigest()
pad_hashes={}
for layout in sorted({case['Layout']for case in cases}):
 path='StepManiaLibrary/'+layout+'.json';data=(a.reference_root/path).read_bytes();assert data==subprocess.check_output(['git','-C',str(a.reference_root),'show',f'{LIB}:{path}']);pad_hashes[path]=sha(data)

with tempfile.TemporaryDirectory(prefix='deadsync-selected-transform-build-')as d:
 root=Path(d);(root/'Adapter.cs').write_text(base);(root/'Program.cs').write_bytes((project/'Program.cs').read_bytes());(root/'request.json').write_text(json.dumps(request))
 for name,text in actions.items():(root/(name+'.cs')).write_text(text)
 (root/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{a.reference_root}/Fumen/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference><Reference Include="StepManiaLibrary"><HintPath>{a.reference_root}/StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(root/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=root/'bin/Release/net10.0/Oracle.dll';run=subprocess.run([str(a.dotnet),str(assembly),str(root/'request.json'),str(a.reference_root)],capture_output=True,text=True,check=True);expected=json.loads(run.stdout);assembly_hash=sha(assembly.read_bytes())
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=GA,library_commit=LIB,fumen_commit=FUMEN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Whole original spatial and player actions, original ForceAddEvents/ForceAddEventInternal, real SML pad data and Fumen writer/readback. Explicit event container, clone/value adapters and inert timing/notification callbacks. Ownership intentionally uses plain AddEvents and duplicate/overlap readbacks are retained; no GUI or complete EditorChart clock claim.',sources_sha256={name:sha(b)for name,b in sources.items()},pad_data_sha256=pad_hashes,extracted_sha256={name:sha(body.encode())for name,body in pieces.items()},adapter_sha256=sha(base.encode()),program_sha256=sha((project/'Program.cs').read_bytes()),assembly_sha256=assembly_hash,request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} original spatial/owner cases and {len(cases)*2} writer/readbacks')
