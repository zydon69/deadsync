#!/usr/bin/env python3
"""Execute pinned original UI comparers with actual pinned StepManiaLibrary configs.

ImGui pointer plumbing and editor wrapper construction are explicit harness stubs.
Comparator classes and extracted source string methods execute unchanged.
"""
import argparse, hashlib, io, json, subprocess, tarfile, tempfile
from pathlib import Path
SOURCE_PIN='48e18f71655d0f0d86144c63239331833501fda2'
LIBRARY_PIN='bdbc427f09cf23a2ffa6d8aa4dc2fdc755b0a4e8'
FUMEN_PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def capture(args):return subprocess.check_output([str(v) for v in args])
def sha(value):return hashlib.sha256(value).hexdigest()
def block(text,signature):
 start=text.index(signature);opening=text.index('{',start);depth=0
 for pos in range(opening,len(text)):
  if text[pos]=='{':depth+=1
  elif text[pos]=='}':
   depth-=1
   if depth==0:return text[start:pos+1]
 raise ValueError(signature)
def extract(root,pin,path):return capture(['git','-C',root,'show',f'{pin}:{path}'])
def wrapper(text,pattern):
 # Full unchanged method bodies, not a second implementation of string keys.
 if pattern:
  signatures=['public override string ToString()','public string GetAbbreviation()','public string GetStartFootingString()','public string GetEndFootingString()','private void RebuildStringRepresentation()','private void RebuildNoteTypeString()','private void RebuildStepTypeString()','private void RebuildStepTypeCheckPeriodString()','private void RebuildStartingFootString()','private void RebuildStartFootingString()','private void RebuildEndFootingString()','private static string GetPatternConfigStartFootChoiceStr(','private static string GetPatternConfigEndFootChoiceStr(']
  fields=['StringRepresentation','Abbreviation','NoteTypeString','StepTypeString','StepTypeCheckPeriodString','StartingFootString','StartingFootingString','EndingFootingString']
  header='public sealed class EditorPatternConfig {public StepManiaLibrary.PerformedChart.PatternConfig Config=new(); public int PatternType; const int L=0; static int GetMeasureSubdivision(int value)=>value;'
 else:
  signatures=['public override string ToString()','public string GetAbbreviation()','private void RebuildStringRepresentation()','private void RebuildTransitionMinString()','private void RebuildTransitionMaxString()','private void RebuildFacingInwardString()','private void RebuildFacingOutwardString()','private void RebuildLateralSpeedString()','private void RebuildLateralRelativeNPSString()','private void RebuildLateralAbsoluteNPSString()','private void RebuildStepDistanceMinString()','private void RebuildStepStretchMinString()','private void RebuildStepSpeedMinString()']
  fields=['StringRepresentation','Abbreviation','TransitionMinString','TransitionMaxString','FacingInwardLimitString','FacingOutwardLimitString','LateralSpeedString','LateralRelativeNPSString','LateralAbsoluteNPSString','StepDistanceMinString','StepStretchMinString','StepSpeedMinString']
  header='public sealed class EditorPerformedChartConfig {public StepManiaLibrary.PerformedChart.Config Config=new(); public int TravelSpeedMinBPM;'
 methods=[block(text,signature) for signature in signatures]
 common='public Guid Guid; public string Name; const string NotificationNameChanged="NameChanged"; void Notify(string id,object payload){} public void Refresh()=>RebuildStringRepresentation();'
 return header+common+''.join(f'private string {name}="";'for name in fields)+'\n'+'\n'.join(methods)+'\n}',methods

def main():
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['source','library','fumen','output','provenance']:p.add_argument('--'+name,type=Path,required=True)
 p.add_argument('--dotnet',default='dotnet');a=p.parse_args()
 paths=['StepManiaEditor/UI/UIPatternComparer.cs','StepManiaEditor/UI/UIPerformedChartComparer.cs','StepManiaEditor/UI/UIPatternConfigTable.cs','StepManiaEditor/UI/UIPerformedChartConfigTable.cs','StepManiaEditor/AutogenConfig/EditorPatternConfig.cs','StepManiaEditor/AutogenConfig/EditorPerformedChartConfig.cs']
 blobs={path:extract(a.source,SOURCE_PIN,path)for path in paths};texts={path:data.decode('utf-8-sig')for path,data in blobs.items()}
 pattern,pattern_methods=wrapper(texts[paths[4]],True);performed,performed_methods=wrapper(texts[paths[5]],False)
 stubs='''using System;using System.Text;using StepManiaLibrary.PerformedChart;
namespace ImGuiNET {public enum ImGuiSortDirection{None,Ascending,Descending}public unsafe struct SpecsPointer{public void* NativePtr;}public struct ImGuiTableSortSpecsPtr{public SpecsPointer Specs;public int SpecsCount;}}
namespace StepManiaEditor {public static class ImGuiUtils{public struct NativeImGuiTableColumnSortSpecs{public uint ColumnUserID;public byte SortDirection;}}
'''
 for path in paths[2:4]:
  name='UIPatternConfigTable'if'PatternConfigTable'in path else'UIPerformedChartConfigTable';stubs+='public class '+name+'{'+block(texts[path],'public enum Column')+'}\n'
 stubs+='}\nnamespace StepManiaEditor.AutogenConfig{'+pattern+'\n'+performed+'}\n'
 library_archive=capture(['git','-C',a.library,'archive','--format=tar',LIBRARY_PIN,'StepManiaLibrary']);fumen_archive=capture(['git','-C',a.fumen,'archive','--format=tar',FUMEN_PIN,'Fumen'])
 harness=Path(__file__).with_suffix('.cs').read_bytes()
 with tempfile.TemporaryDirectory(prefix='deadsync-profile-comparer-')as folder:
  root=Path(folder)
  for archive,destination in [(library_archive,root/'library'),(fumen_archive,root/'library/Fumen')]:
   with tarfile.open(fileobj=io.BytesIO(archive))as t:t.extractall(destination,filter='data')
  (root/'Stubs.cs').write_text(stubs);(root/'Program.cs').write_bytes(harness)
  for path in paths[:2]:(root/Path(path).name).write_bytes(blobs[path])
  project=root/'Oracle.csproj';project.write_text('''<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><AllowUnsafeBlocks>true</AllowUnsafeBlocks><EnableDefaultCompileItems>false</EnableDefaultCompileItems></PropertyGroup><ItemGroup><Compile Include="Program.cs"/><Compile Include="Stubs.cs"/><Compile Include="UIPatternComparer.cs"/><Compile Include="UIPerformedChartComparer.cs"/><ProjectReference Include="library/StepManiaLibrary/StepManiaLibrary.csproj"/><ProjectReference Include="library/Fumen/Fumen/Fumen.csproj"/></ItemGroup></Project>''')
  build=subprocess.run([a.dotnet,'build',str(project),'--configuration','Release','--nologo','--verbosity','quiet','-p:RestoreIgnoreFailedSources=true'],capture_output=True,text=True)
  if build.returncode:raise RuntimeError(build.stdout+build.stderr)
  run=subprocess.run([a.dotnet,str(root/'bin/Release/net10.0/Oracle.dll')],capture_output=True,text=True)
  if run.returncode:raise RuntimeError(run.stdout+run.stderr)
  result=json.loads(run.stdout)
 if len(result['pattern']['columns'])!=9 or len(result['performed']['columns'])!=12:raise RuntimeError('Incomplete source column matrix')
 fixture={'sourcePin':SOURCE_PIN,'libraryPin':LIBRARY_PIN,'fumenPin':FUMEN_PIN,'culture':'InvariantCulture (Editor.cs431–432)','execution':'original comparer classes unchanged; actual pinned library configs and normalization; original source string method bodies unchanged; explicit wrapper and ImGui transport stubs, not graphical UI',**result}
 rendered=(json.dumps(fixture,ensure_ascii=False,indent=2)+'\n').encode();a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_bytes(rendered)
 provenance={'source_pin':SOURCE_PIN,'library_pin':LIBRARY_PIN,'fumen_pin':FUMEN_PIN,'source_sha256':{path:sha(data)for path,data in blobs.items()},'method_sha256':{'pattern':[sha(method.encode())for method in pattern_methods],'performed':[sha(method.encode())for method in performed_methods]},'library_archive_sha256':sha(library_archive),'fumen_archive_sha256':sha(fumen_archive),'harness_sha256':sha(harness),'stubs_sha256':sha(stubs.encode()),'fixture_sha256':sha(rendered),'dotnet_sdk':capture([a.dotnet,'--version']).decode().strip(),'plumbing_stubs':['ImGui sort spec pointer transport with source table enum declarations unchanged','wrapper fields, GUID/Name assignment, notification no-op; no EditorConfig persistence/history','PatternType is integer measure denominator passed by fixtures; GetMeasureSubdivision is identity for this transport, aligned from actual Config.BeatSubDivision*4','wrapper TravelSpeedMinBPM field assigned directly; library Config fields are actual independently seeded models; no source editor speed setter under test'],'limits':['No initial ImGui sort spec or graphics execution','No full editor profile lifecycle execution','No claim that native selector initial cultural order equals first source table draw'],'columns':21,'pair_comparisons':result['pairComparisons']}
 a.provenance.parent.mkdir(parents=True,exist_ok=True);a.provenance.write_text(json.dumps(provenance,ensure_ascii=False,indent=2)+'\n');print(f"Executed 21 original source columns; {result['pairComparisons']} directed pair comparisons")
if __name__=='__main__':main()
