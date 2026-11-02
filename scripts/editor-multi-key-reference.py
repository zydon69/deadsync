#!/usr/bin/env python3
"""Execute the complete pinned KeyCommandManager for arbitrary-key-set contracts."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
PIN = '48e18f71655d0f0d86144c63239331833501fda2'
SOURCE = 'StepManiaEditor/KeyCommandManager.cs'
def binding(id, *alternatives, repeat=False, blocks=True):
    return dict(id=id, alternatives=[list(keys) for keys in alternatives], repeat=repeat, blocks=blocks)
def case(name, bindings, snapshots):
    return dict(name=name, bindings=bindings, frames=[dict(time=round(i*.1,5),down=list(keys)) for i,keys in enumerate(snapshots)])
def requests():
    out=[]
    for order in itertools.permutations(['A','B','C']):
        held=[]; snapshots=[[]]
        for key in order:held.append(key);snapshots.append(held.copy())
        for release in order:held.remove(release);snapshots.append(held.copy())
        snapshots += [['A','B','C'],[],['C'],['A','B','C'],[]]
        out.append(case('three-ordinary-'+''.join(order),[binding('Three',['A','B','C'])],snapshots))
    out += [
        case('two-ordinary-final-first-and-prefix-repress',[binding('Pair',['A','B'])],[[],['B'],['A','B'],['B'],['A','B'],[],['A'],['A','B'],['A'],[]]),
        case('equal-inputs-preserve-registration-order',[binding('First',['A','B']),binding('Second',['A','B'])],[[],['A'],['A','B'],['B'],[],['A','B'],[]]),
        case('equal-sets-different-final-order',[binding('AB',['A','B']),binding('BA',['B','A'])],[[],['B'],['A','B'],['A'],['A','B'],[]]),
        case('longer-set-wins-simultaneous',[binding('Short',['A','B']),binding('Long',['A','B','C'])],[[],['A','B','C'],['A','B'],['A','B'],[],['A','B'],['A','B','C'],[]]),
        case('earlier-shorter-blocks-later-superset',[binding('Long',['A','B','C']),binding('Short',['A','B'])],[[],['A'],['A','B'],['A','B','C'],['A','C'],['A','B','C'],[],['A','B','C'],[]]),
        case('overlap-without-subset-is-independent',[binding('AB',['A','B']),binding('AC',['A','C'])],[[],['A'],['A','B'],['A','B','C'],['A','C'],[]]),
        case('nonblocking-ordinary-modifier',[binding('Modifier',['A','B'],blocks=False),binding('Lane',['A','B','C'])],[[],['A'],['A','B'],['A','B','C'],['A','C'],[]]),
        case('multiple-ordinary-alternatives-same-id',[binding('Lane',['A','B'],['C','D'])],[[],['A','B'],['A','B','C','D'],['C','D'],[]]),
        case('ordinary-prefix-modifier-final',[binding('ModifierFinal',['A','LeftShift'],blocks=False),binding('Other',['A','LeftShift','B'])],[[],['A'],['A','RightShift'],['A','RightShift','B'],['A','B'],[]]),
        case('modifier-only-general-callback',[binding('ModifierOnly',['LeftControl','LeftShift'])],[[],['RightControl'],['RightControl','RightShift'],['LeftControl','RightControl','LeftShift','RightShift'],['LeftControl','RightShift'],['RightShift'],[]]),
        case('physical-control-vs-super-alternatives',[binding('Lane',['LeftControl','A','B'],['LeftWindows','A','B'])],[[],['LeftControl','LeftWindows','A'],['LeftControl','LeftWindows','A','B'],['LeftWindows','A','B'],['A','B'],[]]),
        case('reserved-rescue-key-is-valid-in-original-engine',[binding('Menu',['F1']),binding('Other',['F1','F13'])],[[],['F13'],['F1','F13'],[],['F1'],['F1','F13'],[]]),
        case('ten-key-set-has-no-eight-key-source-limit',[binding('Ten',['A','B','C','D','F13','F14','D1','S','Enter','LeftShift'])],[[],['A','B','C','D','F13','F14','D1','S','Enter','LeftShift'],['A','B','C','D','F13','F14','D1','S','Enter'],[]]),
        case('repeated-prefix-key-release',[binding('Pair',['A','B'])],[[],['A'],['A','B'],['A'],['A','B'],['B'],['A','B'],[]]),
    ]
    repeated=case('repeat-strict-boundary-arbitrary-set',[binding('Repeat',['A','B'],repeat=True)],[[],['A','B']])
    repeated['frames'] += [dict(time=t,down=['A','B']) for t in [.6,.601,.62,.621,5.0]]+[dict(time=5.1,down=[])]
    out.append(repeated)
    return out
PROGRAM = r'''
using Microsoft.Xna.Framework.Input;
using StepManiaEditor;
using System.Text.Json;
var result=new List<object>();
foreach(var input in JsonDocument.Parse(File.ReadAllText(args[0])).RootElement.EnumerateArray()) {
 var p=Preferences.Instance.PreferencesKeyBinds;p.Bindings.Clear();
 var effects=new List<string>();var manager=new KeyCommandManager();
 foreach(var item in input.GetProperty("bindings").EnumerateArray()) {
  var id=item.GetProperty("id").GetString()!;
  p.Bindings[id]=item.GetProperty("alternatives").EnumerateArray().Select(chord=>chord.EnumerateArray().Select(key=>Enum.Parse<Keys>(key.GetString()!)).ToArray()).ToList();
  manager.Register(new(id,id,()=>effects.Add("activate:"+id),item.GetProperty("repeat").GetBoolean(),()=>effects.Add("release:"+id),item.GetProperty("blocks").GetBoolean()));
 }
 var conflicts=new List<object>();
 foreach(var (id, alternatives) in p.Bindings)foreach(var chord in alternatives)conflicts.Add(new{id,keys=chord.Select(key=>key.ToString()),blocksInput=p.BlocksInput(id),ids=manager.GetConflictingCommands(id,chord)});
 var frames=new List<object>();
 foreach(var frame in input.GetProperty("frames").EnumerateArray()) {
  var time=frame.GetProperty("time").GetDouble();var down=frame.GetProperty("down").EnumerateArray().Select(key=>Enum.Parse<Keys>(key.GetString()!)).ToArray();
  Keyboard.Down=down;effects.Clear();manager.Update(time);frames.Add(new{time,down=down.Select(key=>key.ToString()),effects=effects.ToArray()});
 }
 result.Add(new{name=input.GetProperty("name").GetString(),conflicts,frames});
}
Console.WriteLine(JsonSerializer.Serialize(result));
'''
def main():
    parser=argparse.ArgumentParser(__doc__);parser.add_argument('--source',type=Path,required=True);parser.add_argument('--dotnet',default='dotnet');parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    source=(args.source/SOURCE).read_bytes();pinned=subprocess.check_output(['git','show',f'{PIN}:{SOURCE}'],cwd=args.source)
    if source!=pinned:raise SystemExit('Original manager differs from pinned Git source')
    template=Path(__file__).parent/'editor-key-command-oracle';host=(template/'Program.cs').read_text();host=host[host.index('namespace Microsoft.Xna.Framework.Input'):]
    old='LeftControl,RightControl,LeftShift,RightShift,LeftAlt,RightAlt,LeftWindows,RightWindows,S,Enter'
    host=host.replace(old,old+',A,B,C,D,F1,F13,F14,D1')
    pref_path='StepManiaEditor/Preferences/PreferencesKeyBinds.cs'
    pref_source=subprocess.check_output(['git','show',f'{PIN}:{pref_path}'],cwd=args.source).decode('utf-8-sig')
    marker='public bool BlocksInput(string id)';start=pref_source.index(marker);opening=pref_source.index('{',start);end=opening+1;depth=1
    while depth:
        depth+=(pref_source[end]=='{')-(pref_source[end]=='}');end+=1
    original_policy=pref_source[start:end]
    fields=' public object ArrowModification,ScrollZoom,ScrollSpacing,MouseSelectionAltBehavior,MouseSelectionControlBehavior,MouseSelectionShiftBehavior; '
    host=host.replace('public bool BlocksInput(string id)=>true;',fields+original_policy)
    request=requests()
    with tempfile.TemporaryDirectory(prefix='deadsync-multi-key-') as temp:
        root=Path(temp);shutil.copyfile(template/'Oracle.csproj',root/'Oracle.csproj');(root/'KeyCommandManager.cs').write_bytes(source);(root/'Program.cs').write_text(PROGRAM+host);(root/'request.json').write_text(json.dumps(request))
        process=subprocess.run([args.dotnet,'run','--project',str(root/'Oracle.csproj'),'--verbosity','quiet','--',str(root/'request.json')],check=True,capture_output=True,text=True)
        lines=[line for line in process.stdout.splitlines() if line.startswith('[')]
        if len(lines)!=1:raise SystemExit(process.stdout)
        output=dict(commit=PIN,path=SOURCE,sha256=hashlib.sha256(source).hexdigest(),policyPath=pref_path,policySha256=hashlib.sha256(original_policy.encode()).hexdigest(),method='Unmodified complete original manager and complete original PreferencesKeyBinds.BlocksInput policy; keyboard/preferences storage/observer stubs only. Activation callbacks include repeats; generic alternatives remain separate.',cases=json.loads(lines[0]))
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(output,indent=2)+'\n');args.output.with_suffix('.request.json').write_text(json.dumps(request,indent=2)+'\n')
    print(f'Original source manager: {len(request)} cases, {sum(len(c["frames"]) for c in request)} frames')
if __name__=='__main__':main()
