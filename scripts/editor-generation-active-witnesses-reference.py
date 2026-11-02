#!/usr/bin/env python3
"""Search dedicated active contexts for previously silent one-leaf mutations."""
import argparse,copy,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',required=True,type=Path);p.add_argument('--matrix',required=True,type=Path);p.add_argument('--library',required=True,type=Path);p.add_argument('--dotnet',required=True,type=Path);p.add_argument('--assembly',required=True,type=Path);p.add_argument('--output',required=True,type=Path);a=p.parse_args();fixtures=a.repo/'crates/deadsync-editor/tests/fixtures/grooveauthor';a.output.mkdir(parents=True,exist_ok=True);matrix=json.loads(a.matrix.read_text());results=[]
for root,pin in [(a.library,'bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'),(a.library/'Fumen','0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89')]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
def invoke(name,request):
 path=a.output/(name+'.request.json');path.write_text(json.dumps(request,indent=2)+'\n');run=subprocess.run([str(a.dotnet),str(a.assembly),str(a.library),str(path)],capture_output=True,text=True,timeout=120)
 if run.returncode:(a.output/(name+'.source-error.txt')).write_text(run.stderr);return None
 result=json.loads(run.stdout);(a.output/(name+'.expected.json')).write_text(json.dumps(result,indent=2)+'\n');return result
for row in matrix:
 if row.get('observable_change') is not False:continue
 family=row['family'];path=row['path'];field=path[-1];found=None;attempts=[]
 for seed in ([42] if family=='expression' else [42,9,71]):
  base=json.loads((fixtures/({'expression':'policy-balanced','pattern':'layout-dance-single','performed':'double-context'}[family]+'.request.json')).read_text());group={'expression':'ExpressedConfig','pattern':'PatternConfig','performed':'PerformedConfig'}[family]
  if family=='expression':
   config=base[group]
   if field=='DefaultBracketParsingMethod':config['BracketParsingDetermination']='useDefaultMethod';before,after='balanced','aggressive'
   elif field=='BracketParsingDetermination':config['DefaultBracketParsingMethod']='aggressive';before,after='chooseMethodDynamically','useDefaultMethod'
   elif field=='MinLevelForBrackets':base['Difficulty']=8.5;before,after=9,8
   elif field=='BalancedBracketsPerMinuteForAggressiveBrackets':before,after=10000.0,0.1
   else:before,after=0.0,9999.0
  elif family=='pattern':
   base['Seed']=seed;config=base[group]
   if field=='StartingFootSpecified':config['StartingFootChoice']='specified';before,after=0,1
   elif field.endswith('LaneSpecified'):
    side='Left' if field.startswith('Left') else 'Right';position='Start' if 'Start' in field else 'End';config[side+'Foot'+position+'Choice']='specifiedLane';before,after=(0,1) if side=='Left' else (2,3)
   else:base['FollowingFooting']=[1,2];before,after='automaticSameOrNewLaneAsFollowing','specifiedLane'
  else:
   base['Seed']=seed;config=base[group];config['Facing'].update(MaxInwardPercentage=0.0,MaxOutwardPercentage=0.0);config['Transitions'].update(Enabled=True,StepsPerTransitionMin=0,StepsPerTransitionMax=8,MinimumPadWidth=1,TransitionCutoffPercentage=0.2);base['Nps']=12.0
   if isinstance(row['before'],bool):
    if field=='DistanceTighteningEnabled':config['StepTightening'].update(DistanceMin=0.0,DistanceMax=0.1)
    if field=='StretchTighteningEnabled':config['StepTightening'].update(StretchDistanceMin=0.0,StretchDistanceMax=0.1)
    before,after=True,False
   elif field.startswith('Max') and 'Percentage' in field:before,after=0.0,1.0
   elif field.endswith('PercentageCutoff'):before,after=0.1,0.9
   elif field=='DistanceMin':config['StepTightening']['DistanceMax']=4.0;before,after=0.1,3.0
   elif field=='DistanceMax':config['StepTightening']['DistanceMin']=0.1;before,after=1.0,4.0
   elif field=='StretchDistanceMin':config['StepTightening']['StretchDistanceMax']=4.0;before,after=0.1,3.0
   elif field=='StretchDistanceMax':config['StepTightening']['StretchDistanceMin']=0.1;before,after=1.0,4.0
   elif field=='LongitudinalMinPanelDistance':before,after=-0.125,1.5
   elif field=='RelativeNPS':before,after=0.1,10.0
   elif field=='AbsoluteNPS':before,after=0.1,20.0
   elif field=='Speed':before,after=0.1,10.0
   elif field=='StepsPerTransitionMin':config['Transitions']['StepsPerTransitionMax']=2000;config['Transitions']['TransitionCutoffPercentage']=0.49;base['PatternConfig'].update(LeftFootEndChoice='specifiedLane',LeftFootEndLaneSpecified=4,RightFootEndChoice='specifiedLane',RightFootEndLaneSpecified=7);before,after=0,1000
   elif field=='StepsPerTransitionMax':before,after=0,32
   elif field=='MinimumPadWidth':before,after=1,20
   elif field=='TransitionCutoffPercentage':before,after=0.1,0.49
   else:raise ValueError(field)
  cursor=base[group]
  for key in path[:-1]:cursor=cursor[key]
  cursor[field]=before;mutated=copy.deepcopy(base);cursor=mutated[group]
  for key in path[:-1]:cursor=cursor[key]
  cursor[field]=after;name='active-'+row['name']+'-'+str(seed);left=invoke(name+'-base',base);right=invoke(name+'-mutated',mutated);effect=left is not None and right is not None and left.get('Success')==True and right.get('Success')==True and left!=right;attempts.append(dict(name=name,valid=left is not None and right is not None,observable_change=effect))
  if effect:found=dict(name=name,before=before,after=after);break
 results.append(dict(original=row['name'],family=family,path=path,witness=found,attempts=attempts));(a.output/'matrix.json').write_text(json.dumps(results,indent=2)+'\n');print(row['name'],bool(found),flush=True)

(a.output/'provenance.json').write_text(json.dumps(dict(assembly_sha256=hashlib.sha256(a.assembly.read_bytes()).hexdigest(),library_commit='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8',fumen_commit='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89',witnesses=sum(row['witness']is not None for row in results),notice="Actual original source search, paired active contexts identical except the specified leaf. No Rust executed. Missing witnesses remain open."),indent=2)+'\n')
