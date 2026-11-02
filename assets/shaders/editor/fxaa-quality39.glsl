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
// Pinned source active HLSL5 gather-emulation branch; original Fumen alpha preservation.
float FxaaLuma(vec4 rgba){return rgba.y;}
float clamp01(float x){return clamp(x,0.0,1.0);}
vec4 fxaa_quality39(
    vec2 pos,
    sampler2D tex,
    vec2 fxaaQualityRcpFrame,
    float fxaaQualitySubpix,
    float fxaaQualityEdgeThreshold,
    float fxaaQualityEdgeThresholdMin
) {
    vec2 posM;
    posM.x = pos.x;
    posM.y = pos.y;
            vec4 rgbyM = texture(tex, posM);
            vec4 luma4A = vec4(texture(tex, posM).y);
            vec4 luma4B = vec4(texture(tex, posM + (vec2(-1, -1) * 1)).y);
    float maxSM = max(luma4A.x, rgbyM.y);
    float minSM = min(luma4A.x, rgbyM.y);
    float maxESM = max(luma4A.z, maxSM);
    float minESM = min(luma4A.z, minSM);
    float maxWN = max(luma4B.z, luma4B.x);
    float minWN = min(luma4B.z, luma4B.x);
    float rangeMax = max(maxWN, maxESM);
    float rangeMin = min(minWN, minESM);
    float rangeMaxScaled = rangeMax * fxaaQualityEdgeThreshold;
    float range = rangeMax - rangeMin;
    float rangeMaxClamped = max(fxaaQualityEdgeThresholdMin, rangeMaxScaled);
    bool earlyExit = range < rangeMaxClamped;
    if(earlyExit)
            return rgbyM;
        float lumaNE = FxaaLuma(texture(tex, posM + (vec2(1, -1) * fxaaQualityRcpFrame.xy)));
        float lumaSW = FxaaLuma(texture(tex, posM + (vec2(-1, 1) * fxaaQualityRcpFrame.xy)));
    float lumaNS = luma4B.z + luma4A.x;
    float lumaWE = luma4B.x + luma4A.z;
    float subpixRcpRange = 1.0/range;
    float subpixNSWE = lumaNS + lumaWE;
    float edgeHorz1 = (-2.0 * rgbyM.y) + lumaNS;
    float edgeVert1 = (-2.0 * rgbyM.y) + lumaWE;
    float lumaNESE = lumaNE + luma4A.y;
    float lumaNWNE = luma4B.w + lumaNE;
    float edgeHorz2 = (-2.0 * luma4A.z) + lumaNESE;
    float edgeVert2 = (-2.0 * luma4B.z) + lumaNWNE;
    float lumaNWSW = luma4B.w + lumaSW;
    float lumaSWSE = lumaSW + luma4A.y;
    float edgeHorz4 = (abs(edgeHorz1) * 2.0) + abs(edgeHorz2);
    float edgeVert4 = (abs(edgeVert1) * 2.0) + abs(edgeVert2);
    float edgeHorz3 = (-2.0 * luma4B.x) + lumaNWSW;
    float edgeVert3 = (-2.0 * luma4A.x) + lumaSWSE;
    float edgeHorz = abs(edgeHorz3) + edgeHorz4;
    float edgeVert = abs(edgeVert3) + edgeVert4;
    float subpixNWSWNESE = lumaNWSW + lumaNESE;
    float lengthSign = fxaaQualityRcpFrame.x;
    bool horzSpan = edgeHorz >= edgeVert;
    float subpixA = subpixNSWE * 2.0 + subpixNWSWNESE;
    if(!horzSpan) luma4B.z = luma4B.x;
    if(!horzSpan) luma4A.x = luma4A.z;
    if(horzSpan) lengthSign = fxaaQualityRcpFrame.y;
    float subpixB = (subpixA * (1.0/12.0)) - rgbyM.y;
    float gradientN = luma4B.z - rgbyM.y;
    float gradientS = luma4A.x - rgbyM.y;
    float lumaNN = luma4B.z + rgbyM.y;
    float lumaSS = luma4A.x + rgbyM.y;
    bool pairN = abs(gradientN) >= abs(gradientS);
    float gradient = max(abs(gradientN), abs(gradientS));
    if(pairN) lengthSign = -lengthSign;
    float subpixC = clamp01(abs(subpixB) * subpixRcpRange);
    vec2 posB;
    posB.x = posM.x;
    posB.y = posM.y;
    vec2 offNP;
    offNP.x = (!horzSpan) ? 0.0 : fxaaQualityRcpFrame.x;
    offNP.y = ( horzSpan) ? 0.0 : fxaaQualityRcpFrame.y;
    if(!horzSpan) posB.x += lengthSign * 0.5;
    if( horzSpan) posB.y += lengthSign * 0.5;
    vec2 posN;
    posN.x = posB.x - offNP.x * 1.0;
    posN.y = posB.y - offNP.y * 1.0;
    vec2 posP;
    posP.x = posB.x + offNP.x * 1.0;
    posP.y = posB.y + offNP.y * 1.0;
    float subpixD = ((-2.0)*subpixC) + 3.0;
    float lumaEndN = FxaaLuma(texture(tex, posN));
    float subpixE = subpixC * subpixC;
    float lumaEndP = FxaaLuma(texture(tex, posP));
    if(!pairN) lumaNN = lumaSS;
    float gradientScaled = gradient * 1.0/4.0;
    float lumaMM = rgbyM.y - lumaNN * 0.5;
    float subpixF = subpixD * subpixE;
    bool lumaMLTZero = lumaMM < 0.0;
    lumaEndN -= lumaNN * 0.5;
    lumaEndP -= lumaNN * 0.5;
    bool doneN = abs(lumaEndN) >= gradientScaled;
    bool doneP = abs(lumaEndP) >= gradientScaled;
    if(!doneN) posN.x -= offNP.x * 1.0;
    if(!doneN) posN.y -= offNP.y * 1.0;
    bool doneNP = (!doneN) || (!doneP);
    if(!doneP) posP.x += offNP.x * 1.0;
    if(!doneP) posP.y += offNP.y * 1.0;
    if(doneNP) {
        if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
        if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
        if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
        if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
        doneN = abs(lumaEndN) >= gradientScaled;
        doneP = abs(lumaEndP) >= gradientScaled;
        if(!doneN) posN.x -= offNP.x * 1.0;
        if(!doneN) posN.y -= offNP.y * 1.0;
        doneNP = (!doneN) || (!doneP);
        if(!doneP) posP.x += offNP.x * 1.0;
        if(!doneP) posP.y += offNP.y * 1.0;
        if(doneNP) {
            if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
            if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
            if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
            if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if(!doneN) posN.x -= offNP.x * 1.0;
            if(!doneN) posN.y -= offNP.y * 1.0;
            doneNP = (!doneN) || (!doneP);
            if(!doneP) posP.x += offNP.x * 1.0;
            if(!doneP) posP.y += offNP.y * 1.0;
            if(doneNP) {
                if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                doneN = abs(lumaEndN) >= gradientScaled;
                doneP = abs(lumaEndP) >= gradientScaled;
                if(!doneN) posN.x -= offNP.x * 1.0;
                if(!doneN) posN.y -= offNP.y * 1.0;
                doneNP = (!doneN) || (!doneP);
                if(!doneP) posP.x += offNP.x * 1.0;
                if(!doneP) posP.y += offNP.y * 1.0;
                if(doneNP) {
                    if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                    if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                    if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                    if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                    doneN = abs(lumaEndN) >= gradientScaled;
                    doneP = abs(lumaEndP) >= gradientScaled;
                    if(!doneN) posN.x -= offNP.x * 1.5;
                    if(!doneN) posN.y -= offNP.y * 1.5;
                    doneNP = (!doneN) || (!doneP);
                    if(!doneP) posP.x += offNP.x * 1.5;
                    if(!doneP) posP.y += offNP.y * 1.5;
                    if(doneNP) {
                        if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                        if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                        if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                        if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                        doneN = abs(lumaEndN) >= gradientScaled;
                        doneP = abs(lumaEndP) >= gradientScaled;
                        if(!doneN) posN.x -= offNP.x * 2.0;
                        if(!doneN) posN.y -= offNP.y * 2.0;
                        doneNP = (!doneN) || (!doneP);
                        if(!doneP) posP.x += offNP.x * 2.0;
                        if(!doneP) posP.y += offNP.y * 2.0;
                        if(doneNP) {
                            if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                            if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                            if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                            if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                            doneN = abs(lumaEndN) >= gradientScaled;
                            doneP = abs(lumaEndP) >= gradientScaled;
                            if(!doneN) posN.x -= offNP.x * 2.0;
                            if(!doneN) posN.y -= offNP.y * 2.0;
                            doneNP = (!doneN) || (!doneP);
                            if(!doneP) posP.x += offNP.x * 2.0;
                            if(!doneP) posP.y += offNP.y * 2.0;
    if(doneNP) {
        if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
        if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
        if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
        if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
        doneN = abs(lumaEndN) >= gradientScaled;
        doneP = abs(lumaEndP) >= gradientScaled;
        if(!doneN) posN.x -= offNP.x * 2.0;
        if(!doneN) posN.y -= offNP.y * 2.0;
        doneNP = (!doneN) || (!doneP);
        if(!doneP) posP.x += offNP.x * 2.0;
        if(!doneP) posP.y += offNP.y * 2.0;
        if(doneNP) {
            if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
            if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
            if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
            if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if(!doneN) posN.x -= offNP.x * 2.0;
            if(!doneN) posN.y -= offNP.y * 2.0;
            doneNP = (!doneN) || (!doneP);
            if(!doneP) posP.x += offNP.x * 2.0;
            if(!doneP) posP.y += offNP.y * 2.0;
            if(doneNP) {
                if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                doneN = abs(lumaEndN) >= gradientScaled;
                doneP = abs(lumaEndP) >= gradientScaled;
                if(!doneN) posN.x -= offNP.x * 4.0;
                if(!doneN) posN.y -= offNP.y * 4.0;
                doneNP = (!doneN) || (!doneP);
                if(!doneP) posP.x += offNP.x * 4.0;
                if(!doneP) posP.y += offNP.y * 4.0;
                if(doneNP) {
                    if(!doneN) lumaEndN = FxaaLuma(texture(tex, posN.xy));
                    if(!doneP) lumaEndP = FxaaLuma(texture(tex, posP.xy));
                    if(!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
                    if(!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
                    doneN = abs(lumaEndN) >= gradientScaled;
                    doneP = abs(lumaEndP) >= gradientScaled;
                    if(!doneN) posN.x -= offNP.x * 8.0;
                    if(!doneN) posN.y -= offNP.y * 8.0;
                    doneNP = (!doneN) || (!doneP);
                    if(!doneP) posP.x += offNP.x * 8.0;
                    if(!doneP) posP.y += offNP.y * 8.0;
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
    float dstN = posM.x - posN.x;
    float dstP = posP.x - posM.x;
    if(!horzSpan) dstN = posM.y - posN.y;
    if(!horzSpan) dstP = posP.y - posM.y;
    bool goodSpanN = (lumaEndN < 0.0) != lumaMLTZero;
    float spanLength = (dstP + dstN);
    bool goodSpanP = (lumaEndP < 0.0) != lumaMLTZero;
    float spanLengthRcp = 1.0/spanLength;
    bool directionN = dstN < dstP;
    float dst = min(dstN, dstP);
    bool goodSpan = directionN ? goodSpanN : goodSpanP;
    float subpixG = subpixF * subpixF;
    float pixelOffset = (dst * (-spanLengthRcp)) + 0.5;
    float subpixH = subpixG * fxaaQualitySubpix;
    float pixelOffsetGood = goodSpan ? pixelOffset : 0.0;
    float pixelOffsetSubpix = max(pixelOffsetGood, subpixH);
    if(!horzSpan) posM.x += pixelOffsetSubpix * lengthSign;
    if( horzSpan) posM.y += pixelOffsetSubpix * lengthSign;
            return vec4(texture(tex, posM).xyz, rgbyM.w);
}
