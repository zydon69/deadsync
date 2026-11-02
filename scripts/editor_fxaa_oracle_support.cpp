#include <cmath>
#include <algorithm>
#include <vector>
#include <cstdio>
using std::min;using std::max;using std::abs;
struct float2{float x,y;float2():x(0),y(0){}float2(float a,float b):x(a),y(b){}float2 operator+(float2 b)const{return{x+b.x,y+b.y};}float2 operator*(float2 b)const{return{x*b.x,y*b.y};}float2 operator*(float b)const{return{x*b,y*b};}};using int2=float2;
struct float3{float x,y,z;};struct float4{float x,y,z,w;float3 xyz;float4(float a):x(a),y(a),z(a),w(a),xyz{a,a,a}{}float4(float a,float b,float c,float d):x(a),y(b),z(c),w(d),xyz{a,b,c}{}float4(float2 a,float c,float d):float4(a.x,a.y,c,d){}float4(float3 a,float d):float4(a.x,a.y,a.z,d){}};using sampler2D=int;
static unsigned W,H;static std::vector<float4> image;
float4 tex2D(int,float4 p){float x=p.x*W-.5f,y=p.y*H-.5f,ix=floor(x),iy=floor(y),tx=x-ix,ty=y-iy;auto get=[](float x,float y){return image[(unsigned)std::clamp(y,0.f,float(H-1))*W+(unsigned)std::clamp(x,0.f,float(W-1))];};auto a=get(ix,iy),b=get(ix+1,iy),c=get(ix,iy+1),d=get(ix+1,iy+1);auto mix=[&](float a,float b,float c,float d){float top=a+(b-a)*tx,bottom=c+(d-c)*tx;return top+(bottom-top)*ty;};return{mix(a.x,b.x,c.x,d.x),mix(a.y,b.y,c.y,d.y),mix(a.z,b.z,c.z,d.z),mix(a.w,b.w,c.w,d.w)};}
float saturate(float x){return std::clamp(x,0.f,1.f);}float FxaaLuma(float4 rgba){return rgba.y;}
