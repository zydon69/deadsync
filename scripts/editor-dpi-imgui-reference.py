#!/usr/bin/env python3
"""Execute the exact pinned ImGui native library with the real original font.
No window/GPU is created. Default Latin glyph range is sufficient for the ASCII
width probes; this does not claim reproduction of the source Japanese atlas.
"""
from pathlib import Path
import ctypes as C,subprocess,hashlib,json,tempfile,argparse
p=argparse.ArgumentParser();p.add_argument('--imgui-root',type=Path,required=True);p.add_argument('--editor-root',type=Path,required=True);p.add_argument('--dpi-fixture',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();pin='f2812cc9e4955e0496611bde2bcdd34225442b5b';editorpin='48e18f71655d0f0d86144c63239331833501fda2'
subprocess.run(['git','-C',str(a.imgui_root),'cat-file','-e',pin],check=True);assert subprocess.check_output(['git','-C',str(a.editor_root),'rev-parse','HEAD'],text=True).strip()==editorpin
blob=subprocess.check_output(['git','-C',str(a.imgui_root),'show',pin+':deps/cimgui/osx/cimgui.dylib']);font=a.editor_root/'StepManiaEditor/Content/Mplus1Code-Medium.ttf'
class V2(C.Structure):_fields_=[('x',C.c_float),('y',C.c_float)]
class Style(C.Structure):_fields_=[('Alpha',C.c_float),('DisabledAlpha',C.c_float),('WindowPadding',V2),('WindowRounding',C.c_float),('WindowBorderSize',C.c_float),('WindowMinSize',V2),('WindowTitleAlign',V2),('WindowMenuButtonPosition',C.c_int),('ChildRounding',C.c_float),('ChildBorderSize',C.c_float),('PopupRounding',C.c_float),('PopupBorderSize',C.c_float),('FramePadding',V2)]
class IO(C.Structure):_fields_=[('ConfigFlags',C.c_int),('BackendFlags',C.c_int),('DisplaySize',V2),('DeltaTime',C.c_float),('IniSavingRate',C.c_float),('IniFilename',C.c_void_p),('LogFilename',C.c_void_p),('UserData',C.c_void_p),('Fonts',C.c_void_p)]
with tempfile.TemporaryDirectory(prefix='dpi-imgui-native-')as d:
 libpath=Path(d)/'cimgui.dylib';libpath.write_bytes(blob);lib=C.CDLL(str(libpath))
 def call(name,restype,args):f=getattr(lib,name);f.restype=restype;f.argtypes=args;return f
 create=call('igCreateContext',C.c_void_p,[C.c_void_p]);destroy=call('igDestroyContext',None,[C.c_void_p]);getio=call('igGetIO',C.POINTER(IO),[]);getstyle=call('igGetStyle',C.POINTER(Style),[]);scaleall=call('ImGuiStyle_ScaleAllSizes',None,[C.POINTER(Style),C.c_float]);addfont=call('ImFontAtlas_AddFontFromFileTTF',C.c_void_p,[C.c_void_p,C.c_char_p,C.c_float,C.c_void_p,C.c_void_p]);build=call('ImFontAtlas_Build',C.c_ubyte,[C.c_void_p]);newframe=call('igNewFrame',None,[]);endframe=call('igEndFrame',None,[]);pushfont=call('igPushFont',None,[C.c_void_p]);popfont=call('igPopFont',None,[]);fontsize=call('igGetFontSize',C.c_float,[]);measure=call('igCalcTextSize',None,[C.POINTER(V2),C.c_char_p,C.c_void_p,C.c_ubyte,C.c_float]);result=[]
 for source in json.loads(a.dpi_fixture.read_text()):
  scale=source['First'];ctx=create(None);io=getio();io.contents.IniFilename=None;io.contents.LogFilename=None;io.contents.DisplaySize=V2(854,480);style=getstyle();assert(style.contents.FramePadding.x,style.contents.FramePadding.y)==(4,3)
  scaleall(style,C.c_float(scale));pixels=source['FontPixels'];record={'Scale':scale,'FramePadding':[style.contents.FramePadding.x,style.contents.FramePadding.y],'FontPixels':pixels,'Loaded':False,'Widths':{}}
  if pixels>0:
   f=addfont(io.contents.Fonts,str(font).encode(),pixels,None,None);assert f;assert build(io.contents.Fonts);newframe();pushfont(f);record['Loaded']=True;record['FontSize']=fontsize();record['WidgetHeight']=style.contents.FramePadding.y*2+fontsize()+2
   for text in ['120.000','4/4','0.125','Preview']:
    size=V2();measure(C.byref(size),text.encode(),None,0,-1);record['Widths'][text]=size.x+source['CloseWidth']
   popfont();endframe()
  else:record['WidgetHeight']=0 # original GetMiscEditorEventHeight exits if font not loaded.
  result.append(record);destroy(ctx)
a.output.write_text(json.dumps(result,indent=2)+'\n');a.output.with_suffix('.reference.json').write_text(json.dumps({'EditorCommit':editorpin,'ImGuiCommit':pin,'NativeLibrarySha256':hashlib.sha256(blob).hexdigest(),'FontSha256':hashlib.sha256(font.read_bytes()).hexdigest(),'Scope':'Real pinned cimgui ScaleAllSizes, real original Mplus font at source integer pixel sizes, CPU atlas and original width/height expressions. Default Latin range only for ASCII probes. No GPU/window or source widget selection execution.'},indent=2)+'\n');print('Executed',len(result),'pinned native ImGui DPI/font/metric cases.')
