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
//! NVIDIA FXAA 3.11 preset 39, pinned Fumen HLSL5 gather emulation.
//! See assets/shaders/editor/fxaa-source.fx for original copyright and notices.
#![allow(non_snake_case, unused_mut, unused_parens, unused_assignments)]
use crate::FxaaQuality;
#[derive(Clone, Copy, Default)]
struct V2 {
    x: f32,
    y: f32,
}
impl std::ops::Add for V2 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            x: self.x + b.x,
            y: self.y + b.y,
        }
    }
}
#[derive(Clone, Copy)]
struct V4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}
impl V4 {
    fn splat(x: f32) -> Self {
        Self {
            x,
            y: x,
            z: x,
            w: x,
        }
    }
}
fn with_alpha(v: V4, w: f32) -> V4 {
    V4 { w, ..v }
}
fn luma(v: V4) -> f32 {
    v.y
}
fn min(a: f32, b: f32) -> f32 {
    a.min(b)
}
fn max(a: f32, b: f32) -> f32 {
    a.max(b)
}
fn abs(a: f32) -> f32 {
    a.abs()
}
fn clamp01(a: f32) -> f32 {
    a.clamp(0.0, 1.0)
}
/// Input/output must hold exactly width*height RGBA texels. No allocation or aliasing.
/// Bilinear normalized sampling clamps to the texture edge. Alpha remains center alpha.
pub fn apply_rgba(
    width: u32,
    height: u32,
    source: &[[f32; 4]],
    destination: &mut [[f32; 4]],
    quality: FxaaQuality,
) {
    apply_rgba_resized(width, height, width, height, source, destination, quality);
}
/// Evaluate at destination texel centers using the original source dimensions for FXAA offsets.
pub fn apply_rgba_resized(
    width: u32,
    height: u32,
    destination_width: u32,
    destination_height: u32,
    source: &[[f32; 4]],
    destination: &mut [[f32; 4]],
    quality: FxaaQuality,
) {
    let size = (width as usize)
        .checked_mul(height as usize)
        .expect("FXAA dimensions overflow");
    let output_size = (destination_width as usize)
        .checked_mul(destination_height as usize)
        .expect("FXAA dimensions overflow");
    assert_eq!(source.len(), size);
    assert_eq!(destination.len(), output_size);
    if output_size == 0 {
        return;
    }
    assert!(size != 0, "FXAA requires a nonempty source");
    let sample = |p: V2| {
        let x = p.x * width as f32 - 0.5;
        let y = p.y * height as f32 - 0.5;
        let ix = x.floor();
        let iy = y.floor();
        let tx = x - ix;
        let ty = y - iy;
        let get = |xx: f32, yy: f32| {
            source[yy.clamp(0.0, height as f32 - 1.0) as usize * width as usize
                + xx.clamp(0.0, width as f32 - 1.0) as usize]
        };
        let a = get(ix, iy);
        let b = get(ix + 1.0, iy);
        let c = get(ix, iy + 1.0);
        let d = get(ix + 1.0, iy + 1.0);
        let v = std::array::from_fn::<_, 4, _>(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bottom = c[i] + (d[i] - c[i]) * tx;
            top + (bottom - top) * ty
        });
        V4 {
            x: v[0],
            y: v[1],
            z: v[2],
            w: v[3],
        }
    };
    for y in 0..destination_height {
        for x in 0..destination_width {
            let v = pixel(
                V2 {
                    x: (x as f32 + 0.5) / destination_width as f32,
                    y: (y as f32 + 0.5) / destination_height as f32,
                },
                V2 {
                    x: 1.0 / width as f32,
                    y: 1.0 / height as f32,
                },
                quality,
                &sample,
            );
            destination[y as usize * destination_width as usize + x as usize] =
                [v.x, v.y, v.z, v.w];
        }
    }
}
fn pixel(pos: V2, fxaaQualityRcpFrame: V2, quality: FxaaQuality, sample: &impl Fn(V2) -> V4) -> V4 {
    let fxaaQualitySubpix = quality.subpix;
    let fxaaQualityEdgeThreshold = quality.edge_threshold;
    let fxaaQualityEdgeThresholdMin = quality.edge_threshold_min;

    let mut posM = V2::default();
    posM.x = pos.x;
    posM.y = pos.y;
    let mut rgbyM = sample(posM);
    let mut luma4A = V4::splat(sample(posM).y);
    let mut luma4B = V4::splat(sample(posM + (V2 { x: -1.0, y: -1.0 })).y);
    let mut maxSM: f32 = max(luma4A.x, rgbyM.y);
    let mut minSM: f32 = min(luma4A.x, rgbyM.y);
    let mut maxESM: f32 = max(luma4A.z, maxSM);
    let mut minESM: f32 = min(luma4A.z, minSM);
    let mut maxWN: f32 = max(luma4B.z, luma4B.x);
    let mut minWN: f32 = min(luma4B.z, luma4B.x);
    let mut rangeMax: f32 = max(maxWN, maxESM);
    let mut rangeMin: f32 = min(minWN, minESM);
    let mut rangeMaxScaled: f32 = rangeMax * fxaaQualityEdgeThreshold;
    let mut range: f32 = rangeMax - rangeMin;
    let mut rangeMaxClamped: f32 = max(fxaaQualityEdgeThresholdMin, rangeMaxScaled);
    let mut earlyExit = range < rangeMaxClamped;
    if earlyExit {
        return rgbyM;
    }
    let mut lumaNE: f32 = luma(sample(
        posM + (V2 {
            x: fxaaQualityRcpFrame.x,
            y: -fxaaQualityRcpFrame.y,
        }),
    ));
    let mut lumaSW: f32 = luma(sample(
        posM + (V2 {
            x: -fxaaQualityRcpFrame.x,
            y: fxaaQualityRcpFrame.y,
        }),
    ));
    let mut lumaNS: f32 = luma4B.z + luma4A.x;
    let mut lumaWE: f32 = luma4B.x + luma4A.z;
    let mut subpixRcpRange: f32 = 1.0 / range;
    let mut subpixNSWE: f32 = lumaNS + lumaWE;
    let mut edgeHorz1: f32 = (-2.0 * rgbyM.y) + lumaNS;
    let mut edgeVert1: f32 = (-2.0 * rgbyM.y) + lumaWE;
    let mut lumaNESE: f32 = lumaNE + luma4A.y;
    let mut lumaNWNE: f32 = luma4B.w + lumaNE;
    let mut edgeHorz2: f32 = (-2.0 * luma4A.z) + lumaNESE;
    let mut edgeVert2: f32 = (-2.0 * luma4B.z) + lumaNWNE;
    let mut lumaNWSW: f32 = luma4B.w + lumaSW;
    let mut lumaSWSE: f32 = lumaSW + luma4A.y;
    let mut edgeHorz4: f32 = (abs(edgeHorz1) * 2.0) + abs(edgeHorz2);
    let mut edgeVert4: f32 = (abs(edgeVert1) * 2.0) + abs(edgeVert2);
    let mut edgeHorz3: f32 = (-2.0 * luma4B.x) + lumaNWSW;
    let mut edgeVert3: f32 = (-2.0 * luma4A.x) + lumaSWSE;
    let mut edgeHorz: f32 = abs(edgeHorz3) + edgeHorz4;
    let mut edgeVert: f32 = abs(edgeVert3) + edgeVert4;
    let mut subpixNWSWNESE: f32 = lumaNWSW + lumaNESE;
    let mut lengthSign: f32 = fxaaQualityRcpFrame.x;
    let mut horzSpan = edgeHorz >= edgeVert;
    let mut subpixA: f32 = subpixNSWE * 2.0 + subpixNWSWNESE;
    if !horzSpan {
        luma4B.z = luma4B.x;
    }
    if !horzSpan {
        luma4A.x = luma4A.z;
    }
    if horzSpan {
        lengthSign = fxaaQualityRcpFrame.y;
    }
    let mut subpixB: f32 = (subpixA * (1.0 / 12.0)) - rgbyM.y;
    let mut gradientN: f32 = luma4B.z - rgbyM.y;
    let mut gradientS: f32 = luma4A.x - rgbyM.y;
    let mut lumaNN: f32 = luma4B.z + rgbyM.y;
    let mut lumaSS: f32 = luma4A.x + rgbyM.y;
    let mut pairN = abs(gradientN) >= abs(gradientS);
    let mut gradient: f32 = max(abs(gradientN), abs(gradientS));
    if pairN {
        lengthSign = -lengthSign;
    }
    let mut subpixC: f32 = clamp01(abs(subpixB) * subpixRcpRange);
    let mut posB = V2::default();
    posB.x = posM.x;
    posB.y = posM.y;
    let mut offNP = V2::default();
    offNP.x = if (!horzSpan) {
        0.0
    } else {
        fxaaQualityRcpFrame.x
    };
    offNP.y = if (horzSpan) {
        0.0
    } else {
        fxaaQualityRcpFrame.y
    };
    if !horzSpan {
        posB.x += lengthSign * 0.5;
    }
    if horzSpan {
        posB.y += lengthSign * 0.5;
    }
    let mut posN = V2::default();
    posN.x = posB.x - offNP.x * 1.0;
    posN.y = posB.y - offNP.y * 1.0;
    let mut posP = V2::default();
    posP.x = posB.x + offNP.x * 1.0;
    posP.y = posB.y + offNP.y * 1.0;
    let mut subpixD: f32 = ((-2.0) * subpixC) + 3.0;
    let mut lumaEndN: f32 = luma(sample(posN));
    let mut subpixE: f32 = subpixC * subpixC;
    let mut lumaEndP: f32 = luma(sample(posP));
    if !pairN {
        lumaNN = lumaSS;
    }
    let mut gradientScaled: f32 = gradient * 1.0 / 4.0;
    let mut lumaMM: f32 = rgbyM.y - lumaNN * 0.5;
    let mut subpixF: f32 = subpixD * subpixE;
    let mut lumaMLTZero = lumaMM < 0.0;
    lumaEndN -= lumaNN * 0.5;
    lumaEndP -= lumaNN * 0.5;
    let mut doneN = abs(lumaEndN) >= gradientScaled;
    let mut doneP = abs(lumaEndP) >= gradientScaled;
    if !doneN {
        posN.x -= offNP.x * 1.0;
    }
    if !doneN {
        posN.y -= offNP.y * 1.0;
    }
    let mut doneNP = (!doneN) || (!doneP);
    if !doneP {
        posP.x += offNP.x * 1.0;
    }
    if !doneP {
        posP.y += offNP.y * 1.0;
    }
    if doneNP {
        if !doneN {
            lumaEndN = luma(sample(posN));
        }
        if !doneP {
            lumaEndP = luma(sample(posP));
        }
        if !doneN {
            lumaEndN = lumaEndN - lumaNN * 0.5;
        }
        if !doneP {
            lumaEndP = lumaEndP - lumaNN * 0.5;
        }
        doneN = abs(lumaEndN) >= gradientScaled;
        doneP = abs(lumaEndP) >= gradientScaled;
        if !doneN {
            posN.x -= offNP.x * 1.0;
        }
        if !doneN {
            posN.y -= offNP.y * 1.0;
        }
        doneNP = (!doneN) || (!doneP);
        if !doneP {
            posP.x += offNP.x * 1.0;
        }
        if !doneP {
            posP.y += offNP.y * 1.0;
        }
        if doneNP {
            if !doneN {
                lumaEndN = luma(sample(posN));
            }
            if !doneP {
                lumaEndP = luma(sample(posP));
            }
            if !doneN {
                lumaEndN = lumaEndN - lumaNN * 0.5;
            }
            if !doneP {
                lumaEndP = lumaEndP - lumaNN * 0.5;
            }
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if !doneN {
                posN.x -= offNP.x * 1.0;
            }
            if !doneN {
                posN.y -= offNP.y * 1.0;
            }
            doneNP = (!doneN) || (!doneP);
            if !doneP {
                posP.x += offNP.x * 1.0;
            }
            if !doneP {
                posP.y += offNP.y * 1.0;
            }
            if doneNP {
                if !doneN {
                    lumaEndN = luma(sample(posN));
                }
                if !doneP {
                    lumaEndP = luma(sample(posP));
                }
                if !doneN {
                    lumaEndN = lumaEndN - lumaNN * 0.5;
                }
                if !doneP {
                    lumaEndP = lumaEndP - lumaNN * 0.5;
                }
                doneN = abs(lumaEndN) >= gradientScaled;
                doneP = abs(lumaEndP) >= gradientScaled;
                if !doneN {
                    posN.x -= offNP.x * 1.0;
                }
                if !doneN {
                    posN.y -= offNP.y * 1.0;
                }
                doneNP = (!doneN) || (!doneP);
                if !doneP {
                    posP.x += offNP.x * 1.0;
                }
                if !doneP {
                    posP.y += offNP.y * 1.0;
                }
                if doneNP {
                    if !doneN {
                        lumaEndN = luma(sample(posN));
                    }
                    if !doneP {
                        lumaEndP = luma(sample(posP));
                    }
                    if !doneN {
                        lumaEndN = lumaEndN - lumaNN * 0.5;
                    }
                    if !doneP {
                        lumaEndP = lumaEndP - lumaNN * 0.5;
                    }
                    doneN = abs(lumaEndN) >= gradientScaled;
                    doneP = abs(lumaEndP) >= gradientScaled;
                    if !doneN {
                        posN.x -= offNP.x * 1.5;
                    }
                    if !doneN {
                        posN.y -= offNP.y * 1.5;
                    }
                    doneNP = (!doneN) || (!doneP);
                    if !doneP {
                        posP.x += offNP.x * 1.5;
                    }
                    if !doneP {
                        posP.y += offNP.y * 1.5;
                    }
                    if doneNP {
                        if !doneN {
                            lumaEndN = luma(sample(posN));
                        }
                        if !doneP {
                            lumaEndP = luma(sample(posP));
                        }
                        if !doneN {
                            lumaEndN = lumaEndN - lumaNN * 0.5;
                        }
                        if !doneP {
                            lumaEndP = lumaEndP - lumaNN * 0.5;
                        }
                        doneN = abs(lumaEndN) >= gradientScaled;
                        doneP = abs(lumaEndP) >= gradientScaled;
                        if !doneN {
                            posN.x -= offNP.x * 2.0;
                        }
                        if !doneN {
                            posN.y -= offNP.y * 2.0;
                        }
                        doneNP = (!doneN) || (!doneP);
                        if !doneP {
                            posP.x += offNP.x * 2.0;
                        }
                        if !doneP {
                            posP.y += offNP.y * 2.0;
                        }
                        if doneNP {
                            if !doneN {
                                lumaEndN = luma(sample(posN));
                            }
                            if !doneP {
                                lumaEndP = luma(sample(posP));
                            }
                            if !doneN {
                                lumaEndN = lumaEndN - lumaNN * 0.5;
                            }
                            if !doneP {
                                lumaEndP = lumaEndP - lumaNN * 0.5;
                            }
                            doneN = abs(lumaEndN) >= gradientScaled;
                            doneP = abs(lumaEndP) >= gradientScaled;
                            if !doneN {
                                posN.x -= offNP.x * 2.0;
                            }
                            if !doneN {
                                posN.y -= offNP.y * 2.0;
                            }
                            doneNP = (!doneN) || (!doneP);
                            if !doneP {
                                posP.x += offNP.x * 2.0;
                            }
                            if !doneP {
                                posP.y += offNP.y * 2.0;
                            }
                            if doneNP {
                                if !doneN {
                                    lumaEndN = luma(sample(posN));
                                }
                                if !doneP {
                                    lumaEndP = luma(sample(posP));
                                }
                                if !doneN {
                                    lumaEndN = lumaEndN - lumaNN * 0.5;
                                }
                                if !doneP {
                                    lumaEndP = lumaEndP - lumaNN * 0.5;
                                }
                                doneN = abs(lumaEndN) >= gradientScaled;
                                doneP = abs(lumaEndP) >= gradientScaled;
                                if !doneN {
                                    posN.x -= offNP.x * 2.0;
                                }
                                if !doneN {
                                    posN.y -= offNP.y * 2.0;
                                }
                                doneNP = (!doneN) || (!doneP);
                                if !doneP {
                                    posP.x += offNP.x * 2.0;
                                }
                                if !doneP {
                                    posP.y += offNP.y * 2.0;
                                }
                                if doneNP {
                                    if !doneN {
                                        lumaEndN = luma(sample(posN));
                                    }
                                    if !doneP {
                                        lumaEndP = luma(sample(posP));
                                    }
                                    if !doneN {
                                        lumaEndN = lumaEndN - lumaNN * 0.5;
                                    }
                                    if !doneP {
                                        lumaEndP = lumaEndP - lumaNN * 0.5;
                                    }
                                    doneN = abs(lumaEndN) >= gradientScaled;
                                    doneP = abs(lumaEndP) >= gradientScaled;
                                    if !doneN {
                                        posN.x -= offNP.x * 2.0;
                                    }
                                    if !doneN {
                                        posN.y -= offNP.y * 2.0;
                                    }
                                    doneNP = (!doneN) || (!doneP);
                                    if !doneP {
                                        posP.x += offNP.x * 2.0;
                                    }
                                    if !doneP {
                                        posP.y += offNP.y * 2.0;
                                    }
                                    if doneNP {
                                        if !doneN {
                                            lumaEndN = luma(sample(posN));
                                        }
                                        if !doneP {
                                            lumaEndP = luma(sample(posP));
                                        }
                                        if !doneN {
                                            lumaEndN = lumaEndN - lumaNN * 0.5;
                                        }
                                        if !doneP {
                                            lumaEndP = lumaEndP - lumaNN * 0.5;
                                        }
                                        doneN = abs(lumaEndN) >= gradientScaled;
                                        doneP = abs(lumaEndP) >= gradientScaled;
                                        if !doneN {
                                            posN.x -= offNP.x * 4.0;
                                        }
                                        if !doneN {
                                            posN.y -= offNP.y * 4.0;
                                        }
                                        doneNP = (!doneN) || (!doneP);
                                        if !doneP {
                                            posP.x += offNP.x * 4.0;
                                        }
                                        if !doneP {
                                            posP.y += offNP.y * 4.0;
                                        }
                                        if doneNP {
                                            if !doneN {
                                                lumaEndN = luma(sample(posN));
                                            }
                                            if !doneP {
                                                lumaEndP = luma(sample(posP));
                                            }
                                            if !doneN {
                                                lumaEndN = lumaEndN - lumaNN * 0.5;
                                            }
                                            if !doneP {
                                                lumaEndP = lumaEndP - lumaNN * 0.5;
                                            }
                                            doneN = abs(lumaEndN) >= gradientScaled;
                                            doneP = abs(lumaEndP) >= gradientScaled;
                                            if !doneN {
                                                posN.x -= offNP.x * 8.0;
                                            }
                                            if !doneN {
                                                posN.y -= offNP.y * 8.0;
                                            }
                                            doneNP = (!doneN) || (!doneP);
                                            if !doneP {
                                                posP.x += offNP.x * 8.0;
                                            }
                                            if !doneP {
                                                posP.y += offNP.y * 8.0;
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
    }
    let mut dstN: f32 = posM.x - posN.x;
    let mut dstP: f32 = posP.x - posM.x;
    if !horzSpan {
        dstN = posM.y - posN.y;
    }
    if !horzSpan {
        dstP = posP.y - posM.y;
    }
    let mut goodSpanN = (lumaEndN < 0.0) != lumaMLTZero;
    let mut spanLength: f32 = (dstP + dstN);
    let mut goodSpanP = (lumaEndP < 0.0) != lumaMLTZero;
    let mut spanLengthRcp: f32 = 1.0 / spanLength;
    let mut directionN = dstN < dstP;
    let mut dst: f32 = min(dstN, dstP);
    let mut goodSpan = if directionN { goodSpanN } else { goodSpanP };
    let mut subpixG: f32 = subpixF * subpixF;
    let mut pixelOffset: f32 = (dst * (-spanLengthRcp)) + 0.5;
    let mut subpixH: f32 = subpixG * fxaaQualitySubpix;
    let mut pixelOffsetGood: f32 = if goodSpan { pixelOffset } else { 0.0 };
    let mut pixelOffsetSubpix: f32 = max(pixelOffsetGood, subpixH);
    if !horzSpan {
        posM.x += pixelOffsetSubpix * lengthSign;
    }
    if horzSpan {
        posM.y += pixelOffsetSubpix * lengthSign;
    }
    return with_alpha(sample(posM), rgbyM.w);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_source_pixels() {
        let (w, h) = (31u32, 19u32);
        let mut image = Vec::new();
        for y in 0..h {
            for x in 0..w {
                image.push([
                    ((x + y) % 7) as f32 / 6.0,
                    ((x * 31 + y * 17) % 101) as f32 / 100.0,
                    ((x * 3 + y) % 11) as f32 / 10.0,
                    ((x + y) % 5) as f32 / 4.0,
                ]);
            }
        }
        let expected = include_bytes!("../tests/fixtures/editor_fxaa_source.expected.bin");
        let mut index = 0;
        for (subpix, edge_threshold, edge_threshold_min) in [
            (0.2, 0.166, 0.0833),
            (0.0, 0.0, 0.0),
            (1.0, 0.01, 0.0),
            (0.75, 0.333, 0.0833),
        ] {
            for (dw, dh) in [(w, h), (7, 11), (53, 23)] {
                let mut output = vec![[0.0; 4]; dw as usize * dh as usize];
                apply_rgba_resized(
                    w,
                    h,
                    dw,
                    dh,
                    &image,
                    &mut output,
                    FxaaQuality {
                        subpix,
                        edge_threshold,
                        edge_threshold_min,
                    },
                );
                for pixel in output {
                    for actual in pixel {
                        let raw =
                            u32::from_le_bytes(expected[index..index + 4].try_into().unwrap());
                        assert_eq!(actual.to_bits(), raw, "component {}", index / 4);
                        index += 4;
                    }
                }
            }
        }
        assert_eq!(index, expected.len());
    }
}
