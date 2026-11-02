#!/usr/bin/env python3
"""Metal readback comparison against independently preprocessed original FXAA HLSL5.

No Cargo or UI. A matching GPU oracle is distinct from CPU/GPU float equivalence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

PIN = "48e18f71655d0f0d86144c63239331833501fda2"
SOURCE_FILE = "StepManiaEditor/Content/fxaa.fx"
PRELUDE = """#include <metal_stdlib>
using namespace metal;
constexpr sampler originalSampler(coord::normalized,address::clamp_to_edge,filter::linear);
float4 tex2D(texture2d<float> tex,float4 coordinate){return tex.sample(originalSampler,coordinate.xy);}
float FxaaLuma(float4 rgba){return rgba.y;}
"""
# Independent fullscreen implementation of the same ABI, not copied from the
# production shader. Positive texture Y is down; Metal clip Y is up.
WRAPPER = """
struct OriginalVertex {float4 position [[position]];float2 uv;};
vertex OriginalVertex fxaa_vertex(uint id [[vertex_id]]) {
    const float4 positions[3]={float4(-1,1,0,1),float4(3,1,0,1),float4(-1,-3,0,1)};
    const float2 coordinates[3]={float2(0,0),float2(2,0),float2(0,2)};
    OriginalVertex out;out.position=positions[id];out.uv=coordinates[id];return out;
}
fragment float4 fxaa_fragment(OriginalVertex in [[stage_in]],texture2d<float> source [[texture(0)]],constant float4 &quality [[buffer(0)]]) {
    float2 inverseSize=float2(1.0/float(source.get_width()),1.0/float(source.get_height()));
    return FxaaPixelShader(in.uv,source,inverseSize,quality.x,quality.y,quality.z);
}
"""
CPP_MAIN = r'''
int main(int argc,char**argv){
 W=31;H=19;FILE* input=fopen(argv[1],"rb");if(!input)return 2;
 for(unsigned i=0;i<W*H;i++){float rgba[4];if(fread(rgba,sizeof(float),4,input)!=4)return 3;image.emplace_back(rgba[0],rgba[1],rgba[2],rgba[3]);}fclose(input);
 for(auto dimensions:std::vector<float2>{{31,19},{7,11},{53,23}}){
  char filename[128];snprintf(filename,sizeof(filename),"cpu-%u-%u.bin",unsigned(dimensions.x),unsigned(dimensions.y));FILE* output=fopen(filename,"wb");if(!output)return 4;
  for(unsigned y=0;y<unsigned(dimensions.y);y++)for(unsigned x=0;x<unsigned(dimensions.x);x++){
   auto a=FxaaPixelShader(float2((x+.5f)/dimensions.x,(y+.5f)/dimensions.y),0,float2(1.f/W,1.f/H),.75f,.166f,.0833f);float values[]={a.x,a.y,a.z,a.w};fwrite(values,sizeof(float),4,output);
  }fclose(output);
 }
}
'''

def digest(data):
    return hashlib.sha256(data).hexdigest()

def run(arguments, **kwargs):
    return subprocess.check_output([str(value) for value in arguments], **kwargs)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--source", type=Path, required=True, help="GrooveAuthor Git checkout containing the pinned commit")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--clangxx", default="clang++")
    parser.add_argument("--swift", default="swift")
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    original = run(["git", "-C", args.source, "show", f"{PIN}:{SOURCE_FILE}"])
    (output / "original.fx").write_bytes(original)
    processed = run([args.clang, "-E", "-P", "-x", "c", output / "original.fx"], text=True)
    (output / "preprocessed-original.fx").write_text(processed)
    body = processed[processed.index("float4 FxaaPixelShader("):processed.index("float4 PixelShaderFunction")]
    # These two scalar splats prove the pinned active HLSL5 branch, including
    # its source offset behavior. Never switch to a corrected gather branch.
    if "float4 luma4A = tex2D(" not in body or "posM + (int2(-1, -1) * 1)" not in body:
        raise RuntimeError("Unexpected active source FXAA preprocessor branch")
    metal_body = body.replace("sampler2D tex", "texture2d<float> tex").replace("int2(", "float2(")
    # Retain the original NVIDIA copyright/license in the generated derivative.
    license_notice = original.decode().split("                           INTEGRATION CHECKLIST", 1)[0] + "*/\n"
    (output / "original-oracle.metal").write_text(license_notice + PRELUDE + metal_body + WRAPPER)
    floats = []
    for y in range(19):
        for x in range(31):
            i = y * 31 + x
            v = 0.9 if x > y else 0.1
            floats.extend([v, ((x * 7 + y * 13) % 29) / 28.0, 1.0 - v, (i % 17) / 16.0])
    # The blue component of source Rust's f32 (1-v) must itself round after v.
    for index in range(0, len(floats), 4):
        v = struct.unpack("<f", struct.pack("<f", floats[index]))[0]
        floats[index + 2] = 1.0 - v
    input_bytes = struct.pack("<" + "f" * len(floats), *floats)
    (output / "input.bin").write_bytes(input_bytes)
    cpp_body = re.sub(r"\.xy(?!z)", "", body)
    cpp_body = re.sub(r"(?<![\w.])(\d+\.\d+)(?![\w.])", r"\1f", cpp_body)
    support = (repo / "scripts/editor_fxaa_oracle_support.cpp").read_text()
    (output / "original-cpu.cpp").write_text(license_notice + support + cpp_body + CPP_MAIN)
    subprocess.run([args.clangxx, "-std=c++17", "-ffp-contract=off", str(output / "original-cpu.cpp"), "-o", str(output / "original-cpu")], check=True)
    subprocess.run([str(output / "original-cpu"), str(output / "input.bin")], cwd=output, check=True)
    production = repo / "crates/deadlib-render-backend-metal/src/shaders/fxaa.metal"
    production_bytes = production.read_bytes()
    production_snapshot = output / "production-under-test.metal"
    production_snapshot.write_bytes(production_bytes)
    result = subprocess.run([args.swift, str(Path(__file__).with_suffix(".swift")), str(output), str(production_snapshot)], text=True, capture_output=True)
    (output / "gpu-comparison.jsonl").write_text(result.stdout)
    (output / "swift.log").write_text(result.stderr)
    manifest = json.loads((Path(__file__).parent / "fixtures/editor-fxaa-metal-source-oracle.json").read_text())
    manifest.update({"source_sha256": digest(original), "original_preprocessed_body_sha256": digest(body.encode()), "production_shader_sha256": digest(production_bytes), "input_sha256": digest(input_bytes), "clang_version": run([args.clang, "--version"], text=True).splitlines()[0], "swift_version": run([args.swift, "--version"], text=True).splitlines()[0], "exit_status": result.returncode})
    (output / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(result.stdout, end="")
    if not result.returncode:
        records = [json.loads(line) for line in result.stdout.splitlines()]
        cases = [record for record in records if "differentBytes" in record]
        if len(cases) != manifest["expected_gpu_cases"] or records[-1] != {"cases": manifest["expected_gpu_cases"], "casesWithDifferentBytes": 0, "totalDifferentBytes": 0}:
            raise RuntimeError("Incomplete or nonmatching GPU readback matrix")
        if any(record["nonfiniteSourceComponents"] or record["differentBytes"] for record in cases):
            raise RuntimeError("Nonfinite or nonmatching GPU readback")
    if result.returncode:
        print(result.stderr)
        raise SystemExit(result.returncode)

if __name__ == "__main__":
    main()
