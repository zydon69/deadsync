#!/usr/bin/env python3
"""Probe the real ICU ABI/formatting parameters against executed UILog fixtures.
This is not Rust execution; the shell Rust differential test is a separate check.
"""
import ctypes as c,ctypes.util,json,argparse,sys
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--expected',required=True);p.add_argument('--library');args=p.parse_args()
lib=c.CDLL(args.library or ('/usr/lib/libicucore.dylib'if sys.platform=='darwin'else ctypes.util.find_library('icui18n')))
suffix=next(s for s in ['']+['_'+str(v)for v in range(255,49,-1)]if hasattr(lib,'udatpg_open'+s))
def fn(name,args,result):
 f=getattr(lib,name+suffix);f.argtypes=args;f.restype=result;return f
ptr=c.c_void_p;I=c.c_int32;U=c.c_uint16;S=c.POINTER(U)
op=fn('udatpg_open',[c.c_char_p,c.POINTER(I)],ptr);cl=fn('udatpg_close',[ptr],None)
best=fn('udatpg_getBestPattern',[ptr,S,I,S,I,c.POINTER(I)],I)
dop=fn('udat_open',[I,I,c.c_char_p,S,I,S,I,c.POINTER(I)],ptr);dcl=fn('udat_close',[ptr],None)
fmt=fn('udat_format',[ptr,c.c_double,S,I,ptr,c.POINTER(I)],I)
def utf(s):
 b=s.encode('utf-16-le');return (U*(len(b)//2)).from_buffer_copy(b)
def read(buf,n):return bytes(buf)[:n*2].decode('utf-16-le')
def clock(locale,style):
 locale=(locale+'@numbers=latn').encode();status=I(0);g=op(locale,c.byref(status));sk=utf('Hms');buf=(U*256)();n=best(g,sk,len(sk),buf,256,c.byref(status));cl(g);assert status.value<=0
 pattern=read(buf,n);start=max(pattern.find('H'),pattern.find('h'),pattern.find('K'),pattern.find('k'))
 end=start
 while end<len(pattern)and pattern[end]==pattern[start]:end+=1
 sep=pattern[end:pattern.find('m',end)].replace("'","")
 assert sep
 pattern=f"HH'{sep}'mm'{sep}'ss"+('.SSS'if style>=2 else'')
 if style==3:pattern='yyyy-MM-dd '+pattern
 p=utf(pattern);z=utf('UTC');status=I(0);handle=dop(-2,-2,locale,z,len(z),p,len(p),c.byref(status));assert handle and status.value<=0
 return handle
cases=json.load(open(args.expected));failed=[]
for case in cases:
 h=clock(case['culture'],case['date'])if case['date']else None;rows=[]
 for level,delta,text in [(0,0,'Info français\nsecond line'),(1,1,'Attention é'),(2,2,'Erreur 😀')]:
  if level<case['level']:continue
  if h:
   status=I(0);buf=(U*256)();n=fmt(h,1793277296123+delta,buf,256,None,c.byref(status));assert status.value<=0;text=read(buf,n)+' '+text
  rows.append(text)
 if h:dcl(h)
 if '\n'.join(rows)!=case['text']:failed.append((case['culture'],case['date'],case['level'],'\n'.join(rows),case['text']))
print('actual ICU probe',len(cases),'cases, mismatches',len(failed))
for x in failed[:3]:print(x)
assert not failed
