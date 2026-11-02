#!/usr/bin/env python3
"""Original Editor.OnToggleDark with full pinned preference models and Fumen.
The chartless host adapters do not render pixels or acquire an audio device.
"""
import importlib.util,json,sys,subprocess
from pathlib import Path
spec=importlib.util.spec_from_file_location('original_groups',Path(__file__).with_name('editor-global-preferences-groups-reference.py'));groups=importlib.util.module_from_spec(spec);spec.loader.exec_module(groups)
def requests(defaults):return[dict(name=f'{show}',raw=json.dumps(dict(Player=3,PreferencesDark=dict(ShowDarkBg=show))))for show in [False,True]]
def main():
 source=Path(sys.argv[sys.argv.index('--source')+1]);editor=subprocess.check_output(['git','-C',str(source),'show',groups.EDITOR_PIN+':StepManiaEditor/Editor.cs']).decode('utf-8-sig');a=editor.index('private void OnToggleDark()');b=editor.index('}',a)+1;body=editor[a:b]
 original='return false;}}';assert original in groups.HOST;groups.HOST=groups.HOST.replace(original,'return false;}'+body+'public void Toggle(){OnToggleDark();}}',1)
 groups.requests=requests;groups.__doc__=__doc__;groups.PROGRAM=r'''using System.Text.Json;using StepManiaEditor;class Program{static void Main(){var output=new List<object>();foreach(var input in JsonDocument.Parse(Console.In.ReadToEnd()).RootElement.EnumerateArray()){var raw=input.GetProperty("raw").GetString();var path=Path.GetTempFileName();try{File.WriteAllText(path,raw);var editor=new Editor();Preferences.Load(editor,path);void Capture(string stage){var p=Preferences.Instance.PreferencesDark;output.Add(new{name=input.GetProperty("name").GetString()+"-"+stage,raw,show=p.ShowDarkBg,value=new{Player=Preferences.Instance.Player,PreferencesDark=new{ShowDarkBg=p.ShowDarkBg}}});}Capture("before");editor.Toggle();Capture("toggle");editor.Toggle();Capture("toggle-back");Preferences.Save(path);Preferences.Load(editor,path);Capture("reopen");}finally{File.Delete(path);}}Console.Write(JsonSerializer.Serialize(new{cases=output}));}}''';groups.main()
if __name__=='__main__':main()
