#!/usr/bin/env python3
"""Run the unchanged original PackSong and comparers against real Fumen readers."""
import argparse,base64,hashlib,json,subprocess
from pathlib import Path
EDITOR_PIN='48e18f71655d0f0d86144c63239331833501fda2'
LIBRARY_PIN='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
FUMEN_PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__)
for name in ['editor-root','reference-root','dotnet','output']:p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();repo=Path(__file__).resolve().parent.parent;project=repo/'scripts/editor-pack-songs-oracle'
for root,pin in [(a.reference_root,LIBRARY_PIN),(a.reference_root/'Fumen',FUMEN_PIN)]:
 assert subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
paths={'pack_song':(a.editor_root,EDITOR_PIN,'StepManiaEditor/PackSong.cs'),'pack_load':(a.editor_root,EDITOR_PIN,'StepManiaEditor/PackLoadTask.cs'),'ui_pack':(a.editor_root,EDITOR_PIN,'StepManiaEditor/UI/UIPackProperties.cs'),'reader':(a.reference_root/'Fumen',FUMEN_PIN,'Fumen/Converters/Reader.cs')}
blobs={k:subprocess.check_output(['git','-C',str(root),'show',f'{pin}:{path}'])for k,(root,pin,path)in paths.items()}
(project/'obj').mkdir(exist_ok=True);(project/'obj/OriginalPackSong.cs').write_bytes(blobs['pack_song'])
songs=[]
def add(name,files):songs.append(dict(Name=name,Files=[dict(Name=f,Bytes=base64.b64encode(text.encode()).decode())for f,text in files]))
def sm(title='Title',translit='',charts=None,extra=''):
 charts=charts or [('dance-single','Hard','9')]
 return f'#TITLE:{title};#TITLETRANSLIT:{translit};#ARTIST:Artist;#CREDIT:Author;#BPMS:0=120;'+extra+''.join(f'#NOTES:{kind}::{difficulty}:{meter}::1000;'for kind,difficulty,meter in charts)
def ssc(title='Title',translit='',charts=None,extra=''):
 charts=charts or [('dance-single','Hard','9')]
 return f'#VERSION:0.83;#TITLE:{title};#TITLETRANSLIT:{translit};#ARTIST:Artist;#CREDIT:Author;#BPMS:0=120;'+extra+''.join(f'#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:{meter};#NOTES:1000;'for kind,difficulty,meter in charts)
add('single-sm',[('song.sm',sm('SM title'))]);add('single-ssc',[('song.ssc',ssc('SSC title'))]);add('prefer-ssc',[('song.sm',sm('Ignored')),('song.ssc',ssc('Preferred'))])
add('uppercase-only',[('song.SSC',ssc('Excluded'))]);add('uppercase-and-sm',[('song.SSC',ssc('Ignored uppercase')),('song.sm',sm('Lowercase'))])
add('bad-ssc-with-sm',[('song.sm',sm('No fallback')),('song.ssc','#TITLE:Bad;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:bad;')])
add('no-charts',[('song.ssc','#TITLE:No charts;#ARTIST:Artist;')]);add('missing-title',[('song.sm','#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;')])
add('metadata-without-valid-notes',[('song.ssc',ssc('Metadata only').replace('#NOTES:1000;','#NOTES:not-real-grid;'))])
charts=[('pump-double','Easy','13'),('dance-single','Hard','9'),('dance-single','Beginner','15'),('dance-double','Challenge','12'),('dance-single','Challenge','7'),('dance-single','Easy','7.9'),('unknown-z','Hard','3'),('unknown-a','Easy','2')]
add('ratings-layout-meter',[('song.ssc',ssc('Ratings',charts=charts))]);add('ratings-negative',[('song.sm',sm('Negative',charts=[('dance-single','Hard','-3'),('dance-single','Easy','2.9')]))])
for folder,title,translit in [('z-title-translit','Zulu','Alpha'),('a-title','Alpha',''),('e-accent','Éclair',''),('b-title','beta',''),('same-a','Same',''),('same-z','Same','')]:add(folder,[('song.ssc',ssc(title,translit))])
add('multiple-same-extension',[('a.ssc',ssc('First')),('z.ssc',ssc('Last'))])
for name,meter in [('nan','NaN'),('positive-infinity','Infinity'),('negative-infinity','-Infinity'),('above-int','2147483648'),('below-int','-2147483649'),('very-large','1e30')]:add('rating-'+name,[('song.ssc',ssc('Boundary',charts=[('dance-single','Hard',meter)]))])
grammar=[
 ('thousands','1,000'),('misgroup','1,5'),('consecutive-groups','1,,2'),('leading-group',',1'),('trailing-group','1,'),
 ('group-before-decimal','1,.5'),('group-after-decimal','1.2,3'),('group-before-exponent','1,e2'),('group-in-exponent','1e1,2'),('group-and-exponent','1,2e2'),
 ('only-groups',',,'),('zero-group','0,1'),('plus','+12'),('negative','-12'),('double-minus','--1'),('mixed-signs','+-1'),('space-after-sign','+ 1'),
 ('leading-decimal','.5'),('trailing-decimal','1.'),('fraction','1.5'),('exponent','1e2'),('negative-exponent','1E-2'),('combined',' -1,234.5e+1 '),
 ('special-short-inf','inf'),('special-negative-short-inf','-inf'),('special-plus-short-inf','+inf'),('special-upper-short-inf','INF'),('special-lower-infinity','infinity'),('special-negative-lower-infinity','-infinity'),('special-plus-infinity','+Infinity'),('special-nan','NaN'),('special-lower-nan','nan'),('special-negative-nan','-NaN'),('special-plus-nan','+NaN'),('exponent-overflow','1e999'),('negative-exponent-overflow','-1e999'),
 ('underscore','1_000'),('currency','$1'),('hex','0x10'),('leading-space','  +1'),('surrounding-space',' \t12 \t'),('empty',''),('only-sign','+'),('plus-group','+1,234'),('two-decimals','1.2.3')]
for name,meter in grammar:
 for fmt,builder in [('sm',sm),('ssc',ssc)]:add('grammar-'+fmt+'-'+name,[('song.'+fmt,builder('Grammar',charts=[('dance-single','Hard',meter)]))])
add('mixed-decimal-finite',[('song.ssc',ssc('Finite ordering',charts=[('dance-single','Hard',meter)for meter in ['9.9','9.1','9.2','9.05','9.95']]))])
add('mixed-nan-infinity-zero',[('song.ssc',ssc('Special ordering',charts=[('dance-single','Hard',meter)for meter in ['NaN','0','Infinity','Infinity','-Infinity','-0','1.25','1.5']]))])
request=dict(Songs=songs,Cultures=['en-US','fr-FR'])
a.output.parent.mkdir(parents=True,exist_ok=True);request_path=a.output.with_suffix('.request.json');expected_path=a.output.with_suffix('.expected.json');request_path.write_text(json.dumps(request,indent=2,ensure_ascii=False)+'\n')
subprocess.run([str(a.dotnet),'build',str(a.reference_root/'Fumen/Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
subprocess.run([str(a.dotnet),'build',str(project/'PackSongsOracle.csproj'),'-c','Release','--nologo','-v','quiet',f'-p:ReferenceRoot={a.reference_root}'],check=True)
assembly=project/'bin/Release/net10.0/PackSongsOracle.dll';result=subprocess.run([str(a.dotnet),str(assembly),str(request_path)],capture_output=True,text=True,check=True);expected_path.write_text(json.dumps(json.loads(result.stdout),indent=2,ensure_ascii=False)+'\n')
digest=lambda x:hashlib.sha256(x).hexdigest()
provenance=dict(editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Whole unchanged PackSong.cs, PackChartComparer and PackSongComparer, real Fumen Reader.LoadMetaDataAsync. Program uses original lower-case extension enumeration predicate; full PackLoadTask/UI are reviewed provenance, not executed. Temporary generated files only; cancellation tested before each original load. Enumeration ties are recorded, not declared deterministic across filesystems.',source_blobs={k:dict(path=paths[k][2],git_blob=subprocess.check_output(['git','-C',str(paths[k][0]),'rev-parse',f'{paths[k][1]}:{paths[k][2]}'],text=True).strip(),sha256=digest(v))for k,v in blobs.items()},program_sha256=digest((project/'Program.cs').read_bytes()),assembly_sha256=digest(assembly.read_bytes()),fumen_assembly_sha256=digest((a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()),request_sha256=digest(request_path.read_bytes()),expected_sha256=digest(expected_path.read_bytes()))
a.output.with_suffix('.reference.json').write_text(json.dumps(provenance,indent=2)+'\n')
