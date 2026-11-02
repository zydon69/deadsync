/*============================================================================


                    NVIDIA FXAA 3.11 by TIMOTHY LOTTES


------------------------------------------------------------------------------
COPYRIGHT (C) 2010, 2011 NVIDIA CORPORATION. ALL RIGHTS RESERVED.
------------------------------------------------------------------------------
TO THE MAXIMUM EXTENT PERMITTED BY APPLICABLE LAW, THIS SOFTWARE IS PROVIDED
*AS IS* AND NVIDIA AND ITS SUPPLIERS DISCLAIM ALL WARRANTIES, EITHER EXPRESS
OR IMPLIED, INCLUDING, BUT NOT LIMITED TO, IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL NVIDIA
OR ITS SUPPLIERS BE LIABLE FOR ANY SPECIAL, INCIDENTAL, INDIRECT, OR
CONSEQUENTIAL DAMAGES WHATSOEVER (INCLUDING, WITHOUT LIMITATION, DAMAGES FOR
LOSS OF BUSINESS PROFITS, BUSINESS INTERRUPTION, LOSS OF BUSINESS INFORMATION,
OR ANY OTHER PECUNIARY LOSS) ARISING OUT OF THE USE OF OR INABILITY TO USE
THIS SOFTWARE, EVEN IF NVIDIA HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH
DAMAGES.

*/
// Source FXAA preset39, green-as-luma, active HLSL5 gather emulation.
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var linearSampler: sampler;
@group(0) @binding(2) var<uniform> quality: vec4<f32>;
fn FxaaLuma(rgba: vec4<f32>) -> f32 {return rgba.y;}
fn clamp01(x: f32) -> f32 {return clamp(x,0.0,1.0);}
fn fxaa_quality39(pos: vec2<f32>,fxaaQualityRcpFrame: vec2<f32>,fxaaQualitySubpix: f32,fxaaQualityEdgeThreshold: f32,fxaaQualityEdgeThresholdMin: f32) -> vec4<f32> {
    var posM: vec2<f32>;
    posM.x = pos.x;
    posM.y = pos.y;
            var rgbyM: vec4<f32> = textureSampleLevel(tex, linearSampler, posM, 0.0);
            var luma4A: vec4<f32> = vec4<f32>(textureSampleLevel(tex, linearSampler, posM, 0.0).y);
            var luma4B: vec4<f32> = vec4<f32>(textureSampleLevel(tex, linearSampler, posM + (vec2<f32>(-1, -1) * 1), 0.0).y);
    var maxSM: f32 = max(luma4A.x, rgbyM.y);
    var minSM: f32 = min(luma4A.x, rgbyM.y);
    var maxESM: f32 = max(luma4A.z, maxSM);
    var minESM: f32 = min(luma4A.z, minSM);
    var maxWN: f32 = max(luma4B.z, luma4B.x);
    var minWN: f32 = min(luma4B.z, luma4B.x);
    var rangeMax: f32 = max(maxWN, maxESM);
    var rangeMin: f32 = min(minWN, minESM);
    var rangeMaxScaled: f32 = rangeMax * fxaaQualityEdgeThreshold;
    var range: f32 = rangeMax - rangeMin;
    var rangeMaxClamped: f32 = max(fxaaQualityEdgeThresholdMin, rangeMaxScaled);
    var earlyExit: bool = range < rangeMaxClamped;
    if(earlyExit) {return rgbyM;}
        var lumaNE: f32 = FxaaLuma(textureSampleLevel(tex, linearSampler, posM + (vec2<f32>(1, -1) * fxaaQualityRcpFrame.xy), 0.0));
        var lumaSW: f32 = FxaaLuma(textureSampleLevel(tex, linearSampler, posM + (vec2<f32>(-1, 1) * fxaaQualityRcpFrame.xy), 0.0));
    var lumaNS: f32 = luma4B.z + luma4A.x;
    var lumaWE: f32 = luma4B.x + luma4A.z;
    var subpixRcpRange: f32 = 1.0/range;
    var subpixNSWE: f32 = lumaNS + lumaWE;
    var edgeHorz1: f32 = (-2.0 * rgbyM.y) + lumaNS;
    var edgeVert1: f32 = (-2.0 * rgbyM.y) + lumaWE;
    var lumaNESE: f32 = lumaNE + luma4A.y;
    var lumaNWNE: f32 = luma4B.w + lumaNE;
    var edgeHorz2: f32 = (-2.0 * luma4A.z) + lumaNESE;
    var edgeVert2: f32 = (-2.0 * luma4B.z) + lumaNWNE;
    var lumaNWSW: f32 = luma4B.w + lumaSW;
    var lumaSWSE: f32 = lumaSW + luma4A.y;
    var edgeHorz4: f32 = (abs(edgeHorz1) * 2.0) + abs(edgeHorz2);
    var edgeVert4: f32 = (abs(edgeVert1) * 2.0) + abs(edgeVert2);
    var edgeHorz3: f32 = (-2.0 * luma4B.x) + lumaNWSW;
    var edgeVert3: f32 = (-2.0 * luma4A.x) + lumaSWSE;
    var edgeHorz: f32 = abs(edgeHorz3) + edgeHorz4;
    var edgeVert: f32 = abs(edgeVert3) + edgeVert4;
    var subpixNWSWNESE: f32 = lumaNWSW + lumaNESE;
    var lengthSign: f32 = fxaaQualityRcpFrame.x;
    var horzSpan: bool = edgeHorz >= edgeVert;
    var subpixA: f32 = subpixNSWE * 2.0 + subpixNWSWNESE;
    if(!horzSpan) {luma4B.z = luma4B.x;}
    if(!horzSpan) {luma4A.x = luma4A.z;}
    if(horzSpan) {lengthSign = fxaaQualityRcpFrame.y;}
    var subpixB: f32 = (subpixA * (1.0/12.0)) - rgbyM.y;
    var gradientN: f32 = luma4B.z - rgbyM.y;
    var gradientS: f32 = luma4A.x - rgbyM.y;
    var lumaNN: f32 = luma4B.z + rgbyM.y;
    var lumaSS: f32 = luma4A.x + rgbyM.y;
    var pairN: bool = abs(gradientN) >= abs(gradientS);
    var gradient: f32 = max(abs(gradientN), abs(gradientS));
    if(pairN) {lengthSign = -lengthSign;}
    var subpixC: f32 = clamp01(abs(subpixB) * subpixRcpRange);
    var posB: vec2<f32>;
    posB.x = posM.x;
    posB.y = posM.y;
    var offNP: vec2<f32>;
    offNP.x = select(fxaaQualityRcpFrame.x, 0.0, (!horzSpan));
    offNP.y = select(fxaaQualityRcpFrame.y, 0.0, ( horzSpan));
    if(!horzSpan) {posB.x += lengthSign * 0.5;}
    if( horzSpan) {posB.y += lengthSign * 0.5;}
    var posN: vec2<f32>;
    posN.x = posB.x - offNP.x * 1.0;
    posN.y = posB.y - offNP.y * 1.0;
    var posP: vec2<f32>;
    posP.x = posB.x + offNP.x * 1.0;
    posP.y = posB.y + offNP.y * 1.0;
    var subpixD: f32 = ((-2.0)*subpixC) + 3.0;
    var lumaEndN: f32 = FxaaLuma(textureSampleLevel(tex, linearSampler, posN, 0.0));
    var subpixE: f32 = subpixC * subpixC;
    var lumaEndP: f32 = FxaaLuma(textureSampleLevel(tex, linearSampler, posP, 0.0));
    if(!pairN) {lumaNN = lumaSS;}
    var gradientScaled: f32 = gradient * 1.0/4.0;
    var lumaMM: f32 = rgbyM.y - lumaNN * 0.5;
    var subpixF: f32 = subpixD * subpixE;
    var lumaMLTZero: bool = lumaMM < 0.0;
    lumaEndN -= lumaNN * 0.5;
    lumaEndP -= lumaNN * 0.5;
    var doneN: bool = abs(lumaEndN) >= gradientScaled;
    var doneP: bool = abs(lumaEndP) >= gradientScaled;
    if(!doneN) {posN.x -= offNP.x * 1.0;}
    if(!doneN) {posN.y -= offNP.y * 1.0;}
    var doneNP: bool = (!doneN) || (!doneP);
    if(!doneP) {posP.x += offNP.x * 1.0;}
    if(!doneP) {posP.y += offNP.y * 1.0;}
    if(doneNP) {
        if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
        if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
        if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
        if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
        doneN = abs(lumaEndN) >= gradientScaled;
        doneP = abs(lumaEndP) >= gradientScaled;
        if(!doneN) {posN.x -= offNP.x * 1.0;}
        if(!doneN) {posN.y -= offNP.y * 1.0;}
        doneNP = (!doneN) || (!doneP);
        if(!doneP) {posP.x += offNP.x * 1.0;}
        if(!doneP) {posP.y += offNP.y * 1.0;}
        if(doneNP) {
            if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
            if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
            if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
            if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if(!doneN) {posN.x -= offNP.x * 1.0;}
            if(!doneN) {posN.y -= offNP.y * 1.0;}
            doneNP = (!doneN) || (!doneP);
            if(!doneP) {posP.x += offNP.x * 1.0;}
            if(!doneP) {posP.y += offNP.y * 1.0;}
            if(doneNP) {
                if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                doneN = abs(lumaEndN) >= gradientScaled;
                doneP = abs(lumaEndP) >= gradientScaled;
                if(!doneN) {posN.x -= offNP.x * 1.0;}
                if(!doneN) {posN.y -= offNP.y * 1.0;}
                doneNP = (!doneN) || (!doneP);
                if(!doneP) {posP.x += offNP.x * 1.0;}
                if(!doneP) {posP.y += offNP.y * 1.0;}
                if(doneNP) {
                    if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                    if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                    if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                    if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                    doneN = abs(lumaEndN) >= gradientScaled;
                    doneP = abs(lumaEndP) >= gradientScaled;
                    if(!doneN) {posN.x -= offNP.x * 1.5;}
                    if(!doneN) {posN.y -= offNP.y * 1.5;}
                    doneNP = (!doneN) || (!doneP);
                    if(!doneP) {posP.x += offNP.x * 1.5;}
                    if(!doneP) {posP.y += offNP.y * 1.5;}
                    if(doneNP) {
                        if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                        if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                        if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                        if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                        doneN = abs(lumaEndN) >= gradientScaled;
                        doneP = abs(lumaEndP) >= gradientScaled;
                        if(!doneN) {posN.x -= offNP.x * 2.0;}
                        if(!doneN) {posN.y -= offNP.y * 2.0;}
                        doneNP = (!doneN) || (!doneP);
                        if(!doneP) {posP.x += offNP.x * 2.0;}
                        if(!doneP) {posP.y += offNP.y * 2.0;}
                        if(doneNP) {
                            if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                            if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                            if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                            if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                            doneN = abs(lumaEndN) >= gradientScaled;
                            doneP = abs(lumaEndP) >= gradientScaled;
                            if(!doneN) {posN.x -= offNP.x * 2.0;}
                            if(!doneN) {posN.y -= offNP.y * 2.0;}
                            doneNP = (!doneN) || (!doneP);
                            if(!doneP) {posP.x += offNP.x * 2.0;}
                            if(!doneP) {posP.y += offNP.y * 2.0;}
    if(doneNP) {
        if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
        if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
        if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
        if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
        doneN = abs(lumaEndN) >= gradientScaled;
        doneP = abs(lumaEndP) >= gradientScaled;
        if(!doneN) {posN.x -= offNP.x * 2.0;}
        if(!doneN) {posN.y -= offNP.y * 2.0;}
        doneNP = (!doneN) || (!doneP);
        if(!doneP) {posP.x += offNP.x * 2.0;}
        if(!doneP) {posP.y += offNP.y * 2.0;}
        if(doneNP) {
            if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
            if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
            if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
            if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if(!doneN) {posN.x -= offNP.x * 2.0;}
            if(!doneN) {posN.y -= offNP.y * 2.0;}
            doneNP = (!doneN) || (!doneP);
            if(!doneP) {posP.x += offNP.x * 2.0;}
            if(!doneP) {posP.y += offNP.y * 2.0;}
            if(doneNP) {
                if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                doneN = abs(lumaEndN) >= gradientScaled;
                doneP = abs(lumaEndP) >= gradientScaled;
                if(!doneN) {posN.x -= offNP.x * 4.0;}
                if(!doneN) {posN.y -= offNP.y * 4.0;}
                doneNP = (!doneN) || (!doneP);
                if(!doneP) {posP.x += offNP.x * 4.0;}
                if(!doneP) {posP.y += offNP.y * 4.0;}
                if(doneNP) {
                    if(!doneN) {lumaEndN = FxaaLuma(textureSampleLevel(tex, linearSampler, posN.xy, 0.0));}
                    if(!doneP) {lumaEndP = FxaaLuma(textureSampleLevel(tex, linearSampler, posP.xy, 0.0));}
                    if(!doneN) {lumaEndN = lumaEndN - lumaNN * 0.5;}
                    if(!doneP) {lumaEndP = lumaEndP - lumaNN * 0.5;}
                    doneN = abs(lumaEndN) >= gradientScaled;
                    doneP = abs(lumaEndP) >= gradientScaled;
                    if(!doneN) {posN.x -= offNP.x * 8.0;}
                    if(!doneN) {posN.y -= offNP.y * 8.0;}
                    doneNP = (!doneN) || (!doneP);
                    if(!doneP) {posP.x += offNP.x * 8.0;}
                    if(!doneP) {posP.y += offNP.y * 8.0;}
                }
            }
        }
    }
                        }
                    }
                }
            }
        }
    }
    var dstN: f32 = posM.x - posN.x;
    var dstP: f32 = posP.x - posM.x;
    if(!horzSpan) {dstN = posM.y - posN.y;}
    if(!horzSpan) {dstP = posP.y - posM.y;}
    var goodSpanN: bool = (lumaEndN < 0.0) != lumaMLTZero;
    var spanLength: f32 = (dstP + dstN);
    var goodSpanP: bool = (lumaEndP < 0.0) != lumaMLTZero;
    var spanLengthRcp: f32 = 1.0/spanLength;
    var directionN: bool = dstN < dstP;
    var dst: f32 = min(dstN, dstP);
    var goodSpan: bool = select(goodSpanP, goodSpanN, directionN);
    var subpixG: f32 = subpixF * subpixF;
    var pixelOffset: f32 = (dst * (-spanLengthRcp)) + 0.5;
    var subpixH: f32 = subpixG * fxaaQualitySubpix;
    var pixelOffsetGood: f32 = select(0.0, pixelOffset, goodSpan);
    var pixelOffsetSubpix: f32 = max(pixelOffsetGood, subpixH);
    if(!horzSpan) {posM.x += pixelOffsetSubpix * lengthSign;}
    if( horzSpan) {posM.y += pixelOffsetSubpix * lengthSign;}
            return vec4<f32>(textureSampleLevel(tex, linearSampler, posM, 0.0).xyz, rgbyM.w);
}

struct FxaaVertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn fxaa_vertex(@builtin(vertex_index) id: u32) -> FxaaVertex {
 let points=array<vec2<f32>,3>(vec2<f32>(0.0,0.0),vec2<f32>(2.0,0.0),vec2<f32>(0.0,2.0));
 let uv=points[id];return FxaaVertex(vec4<f32>(uv.x*2.0-1.0,1.0-uv.y*2.0,0.0,1.0),uv);
}
@fragment fn fxaa_fragment(in: FxaaVertex) -> @location(0) vec4<f32> {
 let inverseSize=vec2<f32>(1.0)/vec2<f32>(textureDimensions(tex));
 var color=fxaa_quality39(in.uv,inverseSize,quality.x,quality.y,quality.z);
 if (quality.w != 0.0) {color.a=1.0;}
 return color;
}
