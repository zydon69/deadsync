#!/usr/bin/env python3
"""First-frame profile table sort specs from the actual pinned cimgui library.
No GPU, renderer backend, input interaction or ImGui ini state. NOT run by author.
"""
import argparse,ctypes as C,hashlib,json,platform,re,subprocess,tempfile
from pathlib import Path
GA='48e18f71655d0f0d86144c63239331833501fda2'
IMGUI='f2812cc9e4955e0496611bde2bcdd34225442b5b'
def git(root,ref,path):return subprocess.check_output(['git','-C',str(root),'show',ref+':'+path])
def sha(data):return hashlib.sha256(data).hexdigest()
def flags(text):return {name:int(value)for name,value in re.findall(r'\b(\w+)\s*=\s*(-?\d+)\s*,',text)}
class V2(C.Structure):_fields_=[('x',C.c_float),('y',C.c_float)]
# Exact front prefix of pinned Generated/ImGuiIO.gen.cs, native alignment.
class IO(C.Structure):_fields_=[('ConfigFlags',C.c_int),('BackendFlags',C.c_int),('DisplaySize',V2),('DeltaTime',C.c_float),('IniSavingRate',C.c_float),('IniFilename',C.c_void_p),('LogFilename',C.c_void_p),('UserData',C.c_void_p),('Fonts',C.c_void_p)]
# Source ImGuiUtils.NativeImGuiTableColumnSortSpecs explicit offsets0/4/6/8,size12.
class ColumnSpec(C.Structure):_fields_=[('user',C.c_uint),('index',C.c_short),('order',C.c_short),('direction',C.c_ubyte)]
class Specs(C.Structure):_fields_=[('items',C.POINTER(ColumnSpec)),('count',C.c_int),('dirty',C.c_ubyte)]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--imgui',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 tree=subprocess.check_output(['git','-C',str(a.source),'ls-tree',GA,'ImGui.NET'],text=True)
 if IMGUI not in tree:raise RuntimeError('Pinned GrooveAuthor does not reference expected ImGui.NET gitlink')
 nativepath='deps/cimgui/osx/cimgui.dylib'if platform.system()=='Darwin'else'deps/cimgui/linux-x64/cimgui.so'
 native=git(a.imgui,IMGUI,nativepath)
 sources={}
 def source(name):
  raw=git(a.source,GA,'StepManiaEditor/'+name);sources[name]=sha(raw);return raw.decode('utf-8-sig')
 utils=source('ImGui/ImGuiUtils.cs')
 if 'WidthOrWeight = 0.0f;'not in utils or 'ImGui.TableSetupColumn(colData.Name, colData.Flags, colData.WidthOrWeight, i);'not in utils:raise RuntimeError('Source table setup boundary changed')
 enumraw={name:git(a.imgui,IMGUI,'src/ImGui.NET/Generated/'+name+'.gen.cs')for name in ['ImGuiTableFlags','ImGuiTableColumnFlags','ImGuiTableRowFlags','ImGuiCond','ImGuiIO','ImGuiNative','ImGuiTableSortSpecs']}
 tf=flags(enumraw['ImGuiTableFlags'].decode());cf=flags(enumraw['ImGuiTableColumnFlags'].decode());rf=flags(enumraw['ImGuiTableRowFlags'].decode());cond=flags(enumraw['ImGuiCond'].decode())
 io=enumraw['ImGuiIO'].decode();prefix=io[io.index('public ImGuiConfigFlags ConfigFlags;'):io.index('public float FontGlobalScale;')]
 expected=['ConfigFlags','BackendFlags','DisplaySize','DeltaTime','IniSavingRate','IniFilename','LogFilename','UserData','Fonts']
 if re.findall(r'public [\w*]+ (\w+);',prefix)!=expected or C.sizeof(ColumnSpec)!=12:raise RuntimeError('Pinned ABI prefix/column stride changed')
 tables=[]
 for kind,filename in [('Pattern','UI/UIPatternConfigTable.cs'),('Performed','UI/UIPerformedChartConfigTable.cs')]:
  text=source(filename);body=re.search(r'public enum Column\s*\{(.*?)\}',text,re.S).group(1);columns=re.findall(r'\b(\w+)\s*,',body)
  declarations={}
  for match in re.finditer(r'TableColumnData\[\(int\)Column\.(\w+)\]\s*=\s*new ColumnData\((.*?)\);',text,re.S):
   label=re.search(r'"([^"]*)"',match[2]).group(1);names=re.findall(r'ImGuiTableColumnFlags\.(\w+)',match[2]);declarations[match[1]]={'label':label,'flags':sum(cf[n]for n in names),'flag_names':names}
  if set(declarations)!=set(columns):raise RuntimeError('Not every source column declaration was extracted')
  # Pattern's actual id is "Pattern Configs", Performed is "Performed Chart Configs".
  tablematch=re.search(r'ImGui.BeginTable\("([^"]+)", TableColumnData.Length,(.*?)\)\)',text,re.S)
  names=re.findall(r'ImGuiTableFlags\.(\w+)',tablematch[2]);value=0
  for name in names:value|=tf[name]
  tables.append((kind,tablematch[1],columns,declarations,value,names))
 with tempfile.TemporaryDirectory(prefix='deadsync-profile-first-table-')as tmp:
  library=Path(tmp)/Path(nativepath).name;library.write_bytes(native);lib=C.CDLL(str(library))
  def api(name,result,args):
   f=getattr(lib,name);f.restype=result;f.argtypes=args;return f
  create=api('igCreateContext',C.c_void_p,[C.c_void_p]);destroy=api('igDestroyContext',None,[C.c_void_p]);getio=api('igGetIO',C.POINTER(IO),[])
  font=api('ImFontAtlas_GetTexDataAsRGBA32',None,[C.c_void_p,C.POINTER(C.c_void_p),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_int)])
  newframe=api('igNewFrame',None,[]);render=api('igRender',None,[]);begin=api('igBegin',C.c_ubyte,[C.c_char_p,C.c_void_p,C.c_int]);end=api('igEnd',None,[])
  size=api('igSetNextWindowSize',None,[V2,C.c_int]);position=api('igSetNextWindowPos',None,[V2,C.c_int,V2]);table=api('igBeginTable',C.c_ubyte,[C.c_char_p,C.c_int,C.c_int,V2,C.c_float]);endtable=api('igEndTable',None,[])
  column=api('igTableSetupColumn',None,[C.c_char_p,C.c_int,C.c_float,C.c_uint]);freeze=api('igTableSetupScrollFreeze',None,[C.c_int,C.c_int]);nextrow=api('igTableNextRow',None,[C.c_int,C.c_float]);setcol=api('igTableSetColumnIndex',C.c_ubyte,[C.c_int]);pushid=api('igPushID_Int',None,[C.c_int]);popid=api('igPopID',None,[]);header=api('igTableHeader',None,[C.c_char_p]);sort=api('igTableGetSortSpecs',C.POINTER(Specs),[])
  results=[]
  for kind,title,columns,declarations,tableflags,names in tables:
   context=create(None)
   try:
    state=getio().contents;state.DisplaySize=V2(1280,720);state.DeltaTime=1/60;state.IniFilename=None;state.LogFilename=None
    pixels=C.c_void_p();w=C.c_int();h=C.c_int();bpp=C.c_int();font(state.Fonts,C.byref(pixels),C.byref(w),C.byref(h),C.byref(bpp))
    frames=[]
    for frame in range(2):
     newframe();size(V2(1200,650),cond['Always']);position(V2(10,10),cond['Always'],V2(0,0));visible=begin(b'ProfileTableReference',None,0)
     if not visible:raise RuntimeError('Reference window unexpectedly hidden')
     if not table(title.encode(),len(columns),tableflags,V2(0,0),0):raise RuntimeError('Reference table unexpectedly clipped')
     freeze(0,1)
     for index,name in enumerate(columns):column(declarations[name]['label'].encode(),declarations[name]['flags'],0,index)
     # Intact source header call order; tooltip branch has no hover/input here.
     nextrow(rf['Headers'],0)
     for index,name in enumerate(columns):
      if setcol(index):pushid(index);header(declarations[name]['label'].encode());popid()
     ptr=sort()
     if not ptr:raise RuntimeError('Sortable source table returned null sort specs')
     specs=ptr.contents;items=[{'column_user_id':specs.items[i].user,'column_index':specs.items[i].index,'sort_order':specs.items[i].order,'sort_direction':specs.items[i].direction,'column':columns[specs.items[i].user]}for i in range(specs.count)]
     frames.append({'frame':frame,'dirty':bool(specs.dirty),'specs':items});specs.dirty=0
     endtable();end();render()
    results.append({'kind':kind,'table_id':title,'flags':names,'columns':[dict(name=name,user_id=i,**declarations[name])for i,name in enumerate(columns)],'frames':frames})
   finally:destroy(context)
 provenance={'ga_pin':GA,'imgui_pin':IMGUI,'native_path':nativepath,'native_sha256':sha(native),'source_sha256':sources,'abi_source_sha256':{name:sha(raw)for name,raw in enumraw.items()},'host':platform.platform(),'boundary':'actual pinned cimgui first and second frames; source column ids/flags/labels/zero widths and header sequence; fresh context, no ini/GPU/backend/input, default font atlas; no config comparator invocation in this witness'}
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps({'tables':results,'provenance':provenance},indent=2)+'\n');print(json.dumps(results,indent=2))
if __name__=='__main__':main()
