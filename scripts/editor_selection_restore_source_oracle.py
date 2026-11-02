#!/usr/bin/env python3
"""Execute original Selection defaults, Restore action and save/reopen with real Fumen.
Full pinned preference models load chartless through the existing sibling host;
no native UI, pointer gesture, or selection membership behavior is simulated.
"""
import importlib.util,json,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('original_groups',Path(__file__).with_name('editor-global-preferences-groups-reference.py'));groups=importlib.util.module_from_spec(spec);spec.loader.exec_module(groups)
def requests(defaults):return[dict(name=f'{mode}-{region}',raw=json.dumps(dict(Player=3,PreferencesSelection=dict(Mode=mode,RegionMode=region))))for mode in['OverlapAny','OverlapCenter','OverlapAll']for region in['TimeOrPosition','TimeOrPositionAndLane']]
groups.PROGRAM='''using System.Text.Json;using System.Reflection;using StepManiaEditor;using Fumen;
class Program{static void Main(){var options=(JsonSerializerOptions)typeof(Preferences).GetField("SerializationOptions",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=System.IO.Path.GetTempFileName();try{File.WriteAllText(path,raw);Preferences.Load(new Editor(),path);void Capture(string stage){output.Add(new{name=input.GetProperty("name").GetString()+"-"+stage,raw,using_defaults=Preferences.Instance.PreferencesSelection.IsUsingDefaults(),value=JsonSerializer.SerializeToElement(Preferences.Instance,options)});}Capture("before");var action=new ActionRestoreSelectionPreferenceDefaults();action.Do();Capture("restore");action.Undo();Capture("undo");action.Do();Capture("redo");Preferences.Save(path);Preferences.Load(new Editor(),path);Capture("reopen");}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{cases=output}));}}
'''
if __name__=='__main__':
 groups.requests=requests;groups.__doc__=__doc__;groups.main();output=Path(sys.argv[sys.argv.index('--output')+1]);payload=json.loads(output.read_text())
 for case in payload['cases']:case['value']={'PreferencesSelection':case['value']['PreferencesSelection']}
 output.write_text(json.dumps(payload,indent=2)+'\n')
