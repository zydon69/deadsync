#!/usr/bin/env python3
"""Execute real pinned Fumen Pack.ini reader/writer and original editor defaults."""
import argparse,base64,hashlib,json,subprocess
from pathlib import Path
EDITOR_PIN="48e18f71655d0f0d86144c63239331833501fda2"
LIBRARY_PIN="bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8"
FUMEN_PIN="0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
a=p.parse_args();repo=Path(__file__).resolve().parent.parent;project=repo/'scripts/editor-pack-oracle'
for root,pin in [(a.reference_root,LIBRARY_PIN),(a.reference_root/'Fumen',FUMEN_PIN)]:
 actual=subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()
 if actual!=pin:raise ValueError(f'Expected {pin}, got {actual}')
 subprocess.run(['git','-C',str(root),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
def blob(root,pin,path):return subprocess.check_output(['git','-C',str(root),'show',f'{pin}:{path}'])
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);depth=0
 for end in range(brace,len(text)):
  if text[end]=='{':depth+=1
  elif text[end]=='}':depth-=1
  if depth==0:return text[start:end+1]
 raise ValueError(signature)
paths={
 'editor_pack':(a.editor_root,EDITOR_PIN,'StepManiaEditor/EditorItgManiaPack.cs'),
 'ui_pack':(a.editor_root,EDITOR_PIN,'StepManiaEditor/UI/UIPackProperties.cs'),
 'fumen_pack':(a.reference_root/'Fumen',FUMEN_PIN,'Fumen/Converters/ItgManiaPack.cs'),
 'fumen_ini':(a.reference_root/'Fumen',FUMEN_PIN,'Fumen/Converters/IniFile.cs'),
}
blobs={name:blob(*source)for name,source in paths.items()}
editor=blobs['editor_pack'].decode('utf-8-sig');ui=blobs['ui_pack'].decode('utf-8-sig')
start=ui.index('SyncOffSetType? packOffset = null;');end=ui.index('packOffset ??= SyncOffSetType.ITG;',start)+len('packOffset ??= SyncOffSetType.ITG;')
pieces={'ORIGINAL_CREATE':member(editor,'public static async Task<EditorItgManiaPack> CreateNewPack('),'ORIGINAL_SAVE_ASYNC':member(editor,'public async Task<bool> SaveAsync()'),'ORIGINAL_DIRTY':member(editor,'public bool HasUnsavedChanges()'),'ORIGINAL_SYNC_DEFAULTS':ui[start:end]}
adapter=(project/'Adapter.cs.template').read_text()
for name,body in pieces.items():adapter=adapter.replace('// '+name,body)
(project/'obj').mkdir(exist_ok=True);(project/'obj/OriginalPackAdapter.cs').write_text(adapter)
loads=[]
def add(name,text,encoding='utf-8',prefix=b''):
 raw=text.encode(encoding)if isinstance(text,str)else text
 loads.append(dict(Name=name,Bytes=base64.b64encode(prefix+raw).decode()))
canonical='[Group]\nVersion=1\nDisplayTitle=Pack français\nTranslitTitle=Pack francais\nSortTitle=Pack\nSeries=Series\nYear=2026\nBanner=../Shared/banner.png\nSyncOffset=ITG\n'
add('canonical',canonical)
add('minimal','[Group]\nVersion=1\n')
add('empty-values','[Group]\nVersion=1\nDisplayTitle=\nTranslitTitle=\nSortTitle=\nSeries=\nYear=\nBanner=\nSyncOffset=\n')
add('unknown-and-crlf','; keep\r\n[Other]\r\nVendor=opaque\r\n[Group]\r\nVersion=1\r\nDisplayTitle=Original\r\nFuture=x=y\r\nYear=2020\r\nSyncOffset=NULL\r\n# trailing\r\n')
add('duplicate-key-first','[Group]\nVersion=1\nDisplayTitle=first\nDisplayTitle=second\nYear=2020\nYear=2040\nSyncOffset=ITG\n')
add('duplicate-group-first','[Group]\nVersion=1\nDisplayTitle=first\n[Group]\nVersion=1\nDisplayTitle=second\n')
add('case-sensitive-keys','[Group]\nVersion=1\ndisplaytitle=lower\nDisplayTitle=Upper\nsyncoffset=ITG\nSyncOffset=NULL\n')
add('trim-keys-preserve-values','[Group]\n Version =1\n DisplayTitle =  A=B  \nYear= +2026 \nSyncOffset= ITG \n')
add('comment-markers',';comment\n#comment\n//comment\n--comment\n[Group]\nVersion=1\nDisplayTitle=Value #not-comment; //kept\n')
for name,text in [('missing-group','Version=1\n'),('wrong-group-case','[group]\nVersion=1\n'),('leading-group-space',' [Group]\nVersion=1\n'),('trailing-group-space','[Group] \nVersion=1\n'),('missing-version','[Group]\nDisplayTitle=Pack\n'),('malformed-version','[Group]\nVersion=no\n'),('overflow-version','[Group]\nVersion=2147483648\n')]:add(name,text)
for version in [0,-1,7]:add(f'version-{version}',canonical.replace('Version=1',f'Version={version}'))
for name,value in [('negative','-17'),('overflow','2147483648'),('spaces',' +2026 '),('bad','not-a-year')]:add('year-'+name,canonical.replace('Year=2026','Year='+value))
for name,value in [('lower','itg'),('numeric0','0'),('numeric1','1'),('numeric7','7'),('numeric-negative','-1'),('overflow','2147483648'),('list','NULL, ITG'),('unknown','other')]:add('sync-'+name,canonical.replace('SyncOffset=ITG','SyncOffset='+value))
add('utf8-bom',canonical,prefix=b'\xef\xbb\xbf')
for name,encoding,prefix in [('utf16le','utf-16-le',b'\xff\xfe'),('utf16be','utf-16-be',b'\xfe\xff'),('utf32le','utf-32-le',b'\xff\xfe\0\0'),('utf32be','utf-32-be',b'\0\0\xfe\xff')]:add(name,canonical,encoding,prefix)
add('invalid-utf8',b'[Group]\nVersion=1\nDisplayTitle=A\xc3(B\nYear=2026\nSyncOffset=ITG\n')
creations=[]
for name,active,preferred,year in [('active-itg',0.009,0.0,2026),('active-null',0.0,0.009,1999),('active-custom-prefer-null',0.002,0.0,2024),('missing-prefer-itg',None,0.009,2026),('fallback-itg',None,0.002,2030),('active-one-ulp-itg',float.fromhex((0.009).hex())+float.fromhex('0x1p-59'),0.0,2026)]:
 creations.append(dict(Name=name,Active=active,Preferred=preferred,Year=year,Title='Pack français',Banner='../Shared/banner.png'))
request=dict(Loads=loads,Creations=creations)
request_path=a.output.with_suffix('.request.json');expected_path=a.output.with_suffix('.expected.json');request_path.parent.mkdir(parents=True,exist_ok=True);request_path.write_text(json.dumps(request,indent=2,ensure_ascii=False)+'\n')
subprocess.run([str(a.dotnet),'build',str(a.reference_root/'Fumen/Fumen/Fumen.csproj'),'--configuration','Release','--nologo','--verbosity','quiet'],check=True)
subprocess.run([str(a.dotnet),'build',str(project/'PackOracle.csproj'),'--configuration','Release','--nologo','--verbosity','quiet',f'-p:ReferenceRoot={a.reference_root}'],check=True)
assembly=project/'bin/Release/net10.0/PackOracle.dll'
result=subprocess.run([str(a.dotnet),str(assembly),str(request_path)],check=True,capture_output=True,text=True);expected=json.loads(result.stdout);expected_path.write_text(json.dumps(expected,indent=2,ensure_ascii=False)+'\n')
digest=lambda value:hashlib.sha256(value).hexdigest()
provenance=dict(editor_commit=EDITOR_PIN,library_commit=LIBRARY_PIN,fumen_commit=FUMEN_PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Actual pinned Fumen ItgManiaPack.LoadAsync/Save/Clone/Matches and IniFile; unchanged original editor CreateNewPack, SaveAsync, HasUnsavedChanges and UI sync-default statements. Explicit owner/watcher/clock adapters, injected UTC year. Real file I/O only in a generated temporary directory. No original GUI or file-watcher behavior.',source_blobs={name:dict(path=paths[name][2],git_blob=subprocess.check_output(['git','-C',str(paths[name][0]),'rev-parse',f'{paths[name][1]}:{paths[name][2]}'],text=True).strip(),sha256=digest(raw))for name,raw in blobs.items()},extracted_sha256={name:digest(body.encode())for name,body in pieces.items()},adapter_sha256=digest(adapter.encode()),program_sha256=digest((project/'Program.cs').read_bytes()),assembly_sha256=digest(assembly.read_bytes()),fumen_assembly_sha256=digest((a.reference_root/'Fumen/Fumen/bin/Release/net10.0/Fumen.dll').read_bytes()),request_sha256=digest(request_path.read_bytes()),expected_sha256=digest(expected_path.read_bytes()))
a.output.with_suffix('.reference.json').write_text(json.dumps(provenance,indent=2)+'\n')
