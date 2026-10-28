#!/usr/bin/env python3
"""Execute original pairwise swap actions and menu factories, then real Fumen writers/readers."""
import argparse,base64,hashlib,json,re,subprocess,tempfile
from pathlib import Path
LIB='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8';PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--reference-root',type=Path,required=True);p.add_argument('--dotnet',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.reference_root/'Fumen'
for folder,pin in [(a.reference_root,LIB),(root,PIN)]:
 assert subprocess.check_output(['git','-C',str(folder),'rev-parse','HEAD'],text=True).strip()==pin
 subprocess.run(['git','-C',str(folder),'diff','--exit-code',pin,'--'],check=True,capture_output=True)
GA='48e18f71655d0f0d86144c63239331833501fda2'
assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==GA
ga_paths=['StepManiaEditor/Editor.cs','StepManiaEditor/EditorSong.cs','StepManiaEditor/EditorChart.cs','StepManiaEditor/Preferences/Preferences.cs']
ga_blobs={path:subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:{path}'])for path in ga_paths}
paths=['Fumen/Converters/SMCommon.cs','Fumen/Converters/SMPropertyParsers.cs','Fumen/Converters/SMReader.cs','Fumen/Converters/SSCReader.cs','Fumen/Converters/MSDFile.cs','Fumen/Converters/SMWriter.cs','Fumen/Converters/SSCWriter.cs','Fumen/Converters/SMWriterBase.cs'];blobs={path:subprocess.check_output(['git','-C',str(root),'show',f'{PIN}:{path}'])for path in paths}
def original(path):return subprocess.check_output(['git','-C',str(a.editor_root),'show',f'{GA}:StepManiaEditor/{path}'],text=True,encoding='utf-8-sig')
def member(text,signature):
 start=text.index(signature);brace=text.index('{',start);depth=1;end=brace+1
 while depth:
  if text[end]=='{':depth+=1
  if text[end]=='}':depth-=1
  end+=1
 return text[start:end]
ui=original('UI/UIEditEvents.cs');config=original('EditorEvents/EventConfig.cs');action=original('EditorActions/ActionChangeNoteType.cs')
methods=['SwapTapsAndMines','SwapTapsAndFakes','SwapTapsAndLifts','SwapMinesAndFakes','SwapMinesAndLifts','SwapHoldsAndRolls','SwapHoldsAndTaps','SwapHoldsAndMines','SwapRollsAndTaps','SwapRollsAndMines']
method_bodies=[member(ui,'private void '+name+'(')for name in methods]
factory_signatures=['public static EventConfig CreateHoldConfig(EditorChart chart, int row','public static EventConfig CreateTapConfig(EditorEvent','public static EventConfig CreateMineConfig(EditorEvent','public static EventConfig CreateMineConfig(EditorChart chart, int row','public static EventConfig CreateFakeNoteConfig(EditorEvent','public static EventConfig CreateLiftNoteConfig(EditorEvent']
factory_bodies=[member(config,sig)for sig in factory_signatures]
project=Path(__file__).resolve().parent/'editor-note-swap-oracle';template=(project/'Adapter.cs.template').read_text();adapter=template.replace('// ORIGINAL_SWAPS','\n'.join(method_bodies)).replace('// ORIGINAL_FACTORIES','\n'.join(factory_bodies))
cases=[]
for method in methods:
 events=[dict(Kind='Tempo',Row=0)]+[dict(Kind=kind,Row=index*96+48,Lane=index%8,Player=index%2,**({'End':index*96+96}if kind in ['Hold','Roll']else{}))for index,kind in enumerate(['Tap','Mine','Fake','Lift','Hold','Roll'])]
 for mode in ['all','subset','reversed','none']:
  chosen=[dict(value,Selected=(mode!='none'and(mode!='subset'or index%2==0)))for index,value in enumerate(events)]
  if mode=='reversed':chosen.reverse()
  cases.append(dict(Name=method+'-'+mode,Method=method,Events=chosen))
request=dict(Cases=cases)
subprocess.run([str(a.dotnet),'build',str(root/'Fumen/Fumen.csproj'),'-c','Release','--nologo','-v','quiet'],check=True)
with tempfile.TemporaryDirectory(prefix='deadsync-note-swap-build-')as directory:
 folder=Path(directory);(folder/'Adapter.cs').write_text(adapter);(folder/'OriginalAction.cs').write_text(action);(folder/'Program.cs').write_bytes((project/'Program.cs').read_bytes());(folder/'request.json').write_text(json.dumps(request));(folder/'Oracle.csproj').write_text(f'<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>{root}/Fumen/bin/Release/net10.0/Fumen.dll</HintPath></Reference><Reference Include="StepManiaLibrary"><HintPath>{a.reference_root}/StepManiaLibrary/bin/Release/net10.0/StepManiaLibrary.dll</HintPath></Reference></ItemGroup></Project>')
 subprocess.run([str(a.dotnet),'build',str(folder/'Oracle.csproj'),'-c','Release','--nologo','-v','quiet'],check=True);assembly=folder/'bin/Release/net10.0/Oracle.dll';output=subprocess.run([str(a.dotnet),str(assembly),str(folder/'request.json')],capture_output=True,text=True);assert output.returncode==0,output.stderr;expected=json.loads(output.stdout);assembly_hash=hashlib.sha256(assembly.read_bytes()).hexdigest()
a.output.parent.mkdir(parents=True,exist_ok=True);rp=a.output.with_suffix('.request.json');ep=a.output.with_suffix('.expected.json');rp.write_text(json.dumps(request,indent=2)+'\n');ep.write_text(json.dumps(expected,indent=2)+'\n');sha=lambda b:hashlib.sha256(b).hexdigest()
a.output.with_suffix('.reference.json').write_text(json.dumps(dict(editor_commit=GA,library_commit=LIB,fumen_commit=PIN,sdk=subprocess.check_output([str(a.dotnet),'--version'],text=True).strip(),scope='Unchanged full ActionChangeNoteType, ten original UI pairwise swap methods and six EventConfig factory bodies. Explicit event/storage/factory-dispatch and inert-clock adapters. Real Fumen SM/SSC serialization and full note readback. No GUI or full EditorChart construction; adapters do not prove attribute preservation or general comparator ties.',original_sources_sha256={name:sha(text.encode())for name,text in [('UI/UIEditEvents.cs',ui),('EditorEvents/EventConfig.cs',config),('EditorActions/ActionChangeNoteType.cs',action)]},swaps_sha256=[sha(text.encode())for text in method_bodies],factories_sha256=[sha(text.encode())for text in factory_bodies],adapter_template_sha256=sha(template.encode()),adapter_sha256=sha(adapter.encode()),program_sha256=sha((project/'Program.cs').read_bytes()),assembly_sha256=assembly_hash,request_sha256=sha(rp.read_bytes()),expected_sha256=sha(ep.read_bytes())),indent=2)+'\n');print(f'Wrote {len(cases)} original note swap cases and {len(cases)*2} writer/readback outcomes')
