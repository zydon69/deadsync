#version 450
layout(set=0,binding=0) uniform sampler2D u_tex;
layout(location=0) in vec2 v_uv;
layout(location=0) out vec4 outColor;
layout(push_constant) uniform FxaaPush {
    vec2 inverse_source;
    float subpix;
    float edge_threshold;
    float edge_threshold_min;
    float opaque;
} pc;
// Build inserts the shared, pinned NVIDIA quality39 kernel verbatim.
// FXAA_QUALITY39_SOURCE
void main() {
    outColor=fxaa_quality39(v_uv,u_tex,pc.inverse_source,pc.subpix,pc.edge_threshold,pc.edge_threshold_min);
    if(pc.opaque>0.5) outColor.a=1.0;
}
