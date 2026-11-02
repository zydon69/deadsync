# Independent source FXAA oracle on Metal

## Reproduce

On macOS with Metal, Swift, clang and clang++:

```sh
python3 scripts/editor_fxaa_metal_source_oracle.py \
  --source /path/to/grooveauthor \
  --output-dir /path/to/oracle-output
```

`--repo` defaults to this repository. `--source` must contain commit `48e18f71655d0f0d86144c63239331833501fda2`; the current checkout contents are not substituted. Tool paths can be overridden by `--clang`, `--clangxx`, `--swift`. No Cargo, application launch or UI is involved.

The driver extracts `StepManiaEditor/Content/fxaa.fx` using `git show`, invokes clang's preprocessor with the original file's defaults, and extracts the active original `FxaaPixelShader` body. It does **not** derive its oracle body through our GLSL, CPU kernel or production Metal translation. The generated derivative retains the original NVIDIA notice.

Necessary HLSL→Metal transport adaptations are listed in the fixture manifest: the sampler parameter becomes `texture2d<float>`; `tex2D(texture,float4)` samples a normalized linear/clamp sampler at `.xy`; integer vector constants are promoted to float vectors for the same mixed arithmetic. Scalar→vector splats, `.xy` expressions, `saturate`, original control flow and the original gather-emulation offset remain in the extracted source. An independently written fullscreen triangle supplies the matching source UV/fragment ABI.

Both pipelines use the same Metal device, queue, source texture data/format, output dimensions/format and compile option `fastMathEnabled=false`. Readbacks are compared byte for byte. No error threshold or corrected sampler is used. The fourth quality component is zero, so the production RGB-target opaque-output flag is inactive; all source alpha values remain part of the comparison.

## Observed result

Apple M4 Pro, macOS 26.6.2 build 25G83, Swift 6.2.3, Apple clang17:

| Matrix dimension | Values |
| --- | --- |
| Input formats | RGBA32float, RGBA16float, RGBA8unorm |
| Output formats | RGBA32float, RGBA16float, RGBA8unorm |
| Output sizes | 31×19, 7×11, 53×23 from a 31×19 input |
| Quality XYZ | (0.75,0.166,0.0833), (0.2,0.166,0.0833), (0,0,0), (1,0.01,0) |
| Result | 108 cases; zero different bytes; maximum error zero; no nonfinite components |

`editor-fxaa-metal-source-oracle.m4pro-observed.json` records hashes, tool versions, matrix, device metadata and diagnostics. The replay output retains the full 108-case JSONL report, original source/preprocessed body, generated Metal oracle and generated independent C++ program. Binary inputs and CPU readbacks are generated in the selected output directory; they are not committed fixtures.

## CPU discrepancy remains explicit

The independently extracted original body also runs through the existing C++ float/vector support with contraction disabled. For the RGBA32float input/output, first quality and three output sizes, its GPU readback differs from its CPU execution by maxima 0.049772203, 0.086895734 and 0.08503586. These are the same indices and values observed with our production Metal shader. The GPU/GPU comparison is exact even at those locations.

This proves that those CPU discrepancies are **also present in the independent original-source Metal execution**, rather than being introduced by our kernel translation. It does not isolate whether sampling precision, interpolation, arithmetic/reciprocal precision or branch sensitivity causes each CPU discrepancy. The script preserves them as diagnostics; it does not relax CPU tests or replace production hardware filtering with a manual sampler.

## Scope

This is source-body and fullscreen sampling parity on the stated Metal device/OS and finite procedural fixture/matrix. It is not universal CPU/GPU equality, a D3D source-driver oracle, proof for other GPU APIs/devices, or a validation of the complete engine's render-target ordering, `preserve` lifecycle, padding or UI composition. Those require their respective backend and application checks. It does not certify global editor completion.
