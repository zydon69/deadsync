#!/usr/bin/env python3
"""Execute the pinned active FXAA HLSL5 body using C++ float/vector support."""
import argparse,pathlib,subprocess,tempfile,re
MAIN='int main(){W=31;H=19;for(unsigned y=0;y<H;y++)for(unsigned x=0;x<W;x++){float g=((x*31+y*17)%101)/100.f;image.emplace_back(((x+y)%7)/6.f,g,((x*3+y)%11)/10.f,((x+y)%5)/4.f);}for(auto q:std::vector<float3>{{.2f,.166f,.0833f},{0,0,0},{1,.01f,0},{.75f,.333f,.0833f}})for(auto dimensions:std::vector<float2>{{float(W),float(H)},{7,11},{53,23}})for(unsigned y=0;y<unsigned(dimensions.y);y++)for(unsigned x=0;x<unsigned(dimensions.x);x++){auto a=FxaaPixelShader(float2((x+.5f)/dimensions.x,(y+.5f)/dimensions.y),0,float2(1.f/W,1.f/H),q.x,q.y,q.z);float v[]={a.x,a.y,a.z,a.w};fwrite(v,sizeof(float),4,stdout);}}'
PIN="48e18f71655d0f0d86144c63239331833501fda2"
p=argparse.ArgumentParser();p.add_argument("--source",required=True,type=pathlib.Path);p.add_argument("--output",required=True,type=pathlib.Path);p.add_argument("--clang",default="clang");p.add_argument("--clangxx",default="clang++");a=p.parse_args()
source=subprocess.check_output(["git","-C",str(a.source),"show",PIN+":StepManiaEditor/Content/fxaa.fx"],text=True)
with tempfile.TemporaryDirectory(prefix="deadsync-fxaa-original-")as directory:
 root=pathlib.Path(directory);original=root/"source.fx";original.write_text(source)
 processed=subprocess.check_output([a.clang,"-E","-P","-x","c",str(original)],text=True)
 body=processed[processed.index("float4 FxaaPixelShader("):processed.index("float4 PixelShaderFunction")]
 body=re.sub(r"\.xy(?!z)","",body)
 body=re.sub(r"(?<![\w.])(\d+\.\d+)(?![\w.])",r"\1f",body)
 support=pathlib.Path(__file__).with_name("editor_fxaa_oracle_support.cpp").read_text()
 program=root/"oracle.cpp";program.write_text(support+body+MAIN)
 binary=root/"oracle";subprocess.run([a.clangxx,"-std=c++17","-ffp-contract=off",str(program),"-o",str(binary)],check=True)
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_bytes(subprocess.check_output([str(binary)]))
