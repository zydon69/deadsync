#!/usr/bin/env python3
"""Original Receptor property setters during full Load/PostLoad, chartless Editor.
Uses sibling original-model oracle; does not claim focused viewport equivalence.
"""
import importlib.util,json,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("original_groups",Path(__file__).with_name("editor-global-preferences-groups-reference.py"));groups=importlib.util.module_from_spec(spec);spec.loader.exec_module(groups)
def requests(defaults):
 result=[]
 for key in ["PositionX","PositionY"]:
  for value in [-2147483648,-1,0,1,2147483647,-2147483649,2147483648]:
   result.append(dict(name=key+"-"+str(value),raw=json.dumps({"Player":3,"PreferencesReceptors":{key:value}})))
 return result
if __name__=="__main__":
 groups.requests=requests;groups.__doc__=__doc__;groups.main()
 output=Path(sys.argv[sys.argv.index("--output")+1]);payload=json.loads(output.read_text())
 for case in payload["cases"]:case["value"]={"PreferencesReceptors":case["value"]["PreferencesReceptors"]}
 output.write_text(json.dumps(payload,indent=2)+"\n")
