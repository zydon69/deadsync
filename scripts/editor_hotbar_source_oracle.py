#!/usr/bin/env python3
"""Run untouched pinned Hotbar snap-lock reconciliation and measure calculation.

No ImGui frame is fabricated. The original conditional mutation block and the
original IsSnapIndexValidForSnapLock/GetMeasure methods execute with explicit
input records; real pinned Fumen supplies subdivision constants. This is not
an oracle for GUI geometry, keyboard publication, clock lookup or audio.
"""
import argparse,hashlib,json,os,struct,subprocess,tempfile
from pathlib import Path
from xml.sax.saxutils import escape
EDITOR_PIN='48e18f71655d0f0d86144c63239331833501fda2'
FUMEN_PIN='0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89'
def block(text,start):
    left=text.index('{',start);depth=0
    for i in range(left,len(text)):
        if text[i]=='{':depth+=1
        elif text[i]=='}':
            depth-=1
            if depth==0:return text[start:i+1]
    raise RuntimeError('Unterminated original method/block')
def source(root,pin,path):
    if subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()!=pin:raise RuntimeError('Unexpected pinned reference '+str(root))
    return subprocess.check_output(['git','-C',str(root),'show',pin+':'+path]).decode('utf-8-sig')
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['editor-root','fumen-root','dotnet','fixture','output']:p.add_argument('--'+key,type=Path,required=True)
    a=p.parse_args();a.output.parent.mkdir(parents=True,exist_ok=True)
    layout=source(a.editor_root,EDITOR_PIN,'StepManiaEditor/ImGui/ImGuiLayoutUtils.cs')
    manager=source(a.editor_root,EDITOR_PIN,'StepManiaEditor/SnapManager.cs')
    chart=source(a.editor_root,EDITOR_PIN,'StepManiaEditor/EditorChart.cs')
    source(a.fumen_root,FUMEN_PIN,'Fumen/Converters/SMCommon.cs')
    mutation=block(layout,layout.index('if (newIndex != snapLockIndex)',layout.index('public static void DrawRowSnapLockLevels(')))
    allowed=block(manager,manager.index('public static bool IsSnapIndexValidForSnapLock('))
    measure=block(chart,chart.index('private static double GetMeasure('))
    dll=a.fumen_root/'Fumen/bin/Release/net10.0/Fumen.dll'
    program='''using System;using System.Linq;using System.Collections.Generic;using System.Text.Json;using static Fumen.Converters.SMCommon;
class Preferences {public static Preferences Instance=new(); public int SnapIndex,SnapLockIndex;}
class SnapManager {'''+allowed+'''}
class Signature {public int row,n,d;public double Measure;public int GetRow()=>row;public int GetNumerator()=>n;public int GetDenominator()=>d;}
class EditorRateAlteringEvent {public Signature signature;public Signature GetTimeSignature()=>signature;}
class Program {'''+measure+'''
static void Main(){var results=new List<object>();var p=Preferences.Instance;var snapLevels=Enumerable.Range(0,ValidDenominators.Length+1).ToList();
for(int level=0;level<snapLevels.Count;level++)for(int old=0;old<snapLevels.Count;old++)for(int next=0;next<snapLevels.Count;next++){
p.SnapIndex=level;p.SnapLockIndex=old;var snapLockIndex=old;var newIndex=next;
'''+mutation+'''
results.Add(new{level,old_lock=old,new_lock=next,result=p.SnapIndex});}
var measures=new List<object>();foreach(var row in new double[]{-192.5,-12.75,0,60.75,191.5,192,200.5,264.25}){
var signature=row<192?new Signature{row=0,n=4,d=4,Measure=0}:new Signature{row=192,n=3,d=8,Measure=1};
var value=GetMeasure(new EditorRateAlteringEvent{signature=signature},row);measures.Add(new{row,bits=BitConverter.DoubleToInt64Bits(value).ToString("x16")});}
Console.WriteLine(JsonSerializer.Serialize(new{lock_cases=results,measure_cases=measures}));}}
'''
    with tempfile.TemporaryDirectory(prefix='deadsync-hotbar-original-')as tmp:
        work=Path(tmp);(work/'Program.cs').write_text(program)
        (work/'Reference.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>disable</Nullable></PropertyGroup><ItemGroup><Reference Include="Fumen"><HintPath>'+escape(str(dll.resolve()))+'</HintPath></Reference></ItemGroup></Project>')
        env=os.environ.copy();env['DOTNET_NOLOGO']='1';env['DOTNET_CLI_TELEMETRY_OPTOUT']='1'
        output=subprocess.check_output([str(a.dotnet),'run','--project',str(work/'Reference.csproj'),'--configuration','Release','--verbosity','quiet'],text=True,env=env)
    actual=json.loads(output.strip().splitlines()[-1]);expected=json.loads(a.fixture.read_text())
    if actual['lock_cases']!=expected:raise RuntimeError('Original snap-lock reconciliation differs from frozen native cases')
    for case in actual['measure_cases']:
        row=case['row'];value=row/192. if row<192 else(row-192.)/72.+1.
        if case['bits']!=struct.pack('>d',value).hex():raise RuntimeError('Original fractional measure bits differ')
    a.output.write_text(json.dumps(actual,indent=2)+'\n')
    provenance={'editor_pin':EDITOR_PIN,'fumen_pin':FUMEN_PIN,'source_sha256':{name:hashlib.sha256(text.encode()).hexdigest()for name,text in [('ImGuiLayoutUtils.cs',layout),('SnapManager.cs',manager),('EditorChart.cs',chart)]},'fumen_dll_sha256':hashlib.sha256(dll.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(a.fixture.read_bytes()).hexdigest(),'original_lock_cases':len(actual['lock_cases']),'original_measure_cases':len(actual['measure_cases']),'boundary':'Original source block and methods, explicit stub input records, actual Fumen constants; not GUI/clock lookup/audio.'}
    a.output.with_suffix('.provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
    print('PASS',len(actual['lock_cases']),'original snap-lock cases,',len(actual['measure_cases']),'measure bit cases')
if __name__=='__main__':main()
