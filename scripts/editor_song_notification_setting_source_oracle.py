#!/usr/bin/env python3
"""Full original Options models and real Fumen bool-field action from UIOptions.
Proves checkbox field/Undo/Redo/save-reopen; chartless UI/GPU host dependencies
are adapters, not an OS notification or native rendering test.
"""
import importlib.util,json
from pathlib import Path
spec=importlib.util.spec_from_file_location('original_groups',Path(__file__).with_name('editor-global-preferences-groups-reference.py'));groups=importlib.util.module_from_spec(spec);spec.loader.exec_module(groups)
def requests(defaults):return[dict(name=f'{value}',raw=json.dumps(dict(Player=3,PreferencesOptions=dict(SuppressExternalSongModificationNotification=value))))for value in [False,True]]
groups.PROGRAM=r'''using System.Text.Json;using StepManiaEditor;using Fumen;class Program{static void Main(){var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=System.IO.Path.GetTempFileName();try{File.WriteAllText(path,raw);Preferences.Load(new Editor(),path);void Capture(string stage){var p=Preferences.Instance.PreferencesOptions;output.Add(new{name=input.GetProperty("name").GetString()+"-"+stage,raw,suppressed=p.SuppressExternalSongModificationNotification,value=new{Player=Preferences.Instance.Player}});}Capture("before");var p=Preferences.Instance.PreferencesOptions;var action=new ActionSetObjectFieldOrPropertyValue<bool>(p,"SuppressExternalSongModificationNotification",!p.SuppressExternalSongModificationNotification,false);action.Do();Capture("toggle");action.Undo();Capture("undo");action.Do();Capture("redo");Preferences.Save(path);Preferences.Load(new Editor(),path);Capture("reopen");}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{cases=output}));}}'''
if __name__=='__main__':groups.requests=requests;groups.__doc__=__doc__;groups.main()
