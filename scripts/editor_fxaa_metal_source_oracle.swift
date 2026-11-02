import Foundation
import Metal
let root=URL(fileURLWithPath:CommandLine.arguments[1])
guard let device=MTLCreateSystemDefaultDevice(),let queue=device.makeCommandQueue() else {fatalError("Metal unavailable")}
let options=MTLCompileOptions();options.fastMathEnabled=false
let sources=[try String(contentsOf:root.appendingPathComponent("original-oracle.metal"),encoding:.utf8),try String(contentsOfFile:CommandLine.arguments[2],encoding:.utf8)]
let libraries=try sources.map{try device.makeLibrary(source:$0,options:options)}
let inputData=try Data(contentsOf:root.appendingPathComponent("input.bin"))
let originalFloats=inputData.withUnsafeBytes{Array($0.bindMemory(to:Float.self))}
let formats:[(String,MTLPixelFormat,Int)]=[("rgba32Float",.rgba32Float,16),("rgba16Float",.rgba16Float,8),("rgba8Unorm",.rgba8Unorm,4)]
let qualities:[[Float]]=[[0.75,0.166,0.0833,0],[0.2,0.166,0.0833,0],[0,0,0,0],[1,0.01,0,0]]
func emit(_ value:[String:Any]){let data=try! JSONSerialization.data(withJSONObject:value,options:[.sortedKeys]);print(String(data:data,encoding:.utf8)!)}
func components(_ bytes:[UInt8],_ format:String)->[Float]{
 if format=="rgba32Float" {return bytes.withUnsafeBytes{Array($0.bindMemory(to:Float.self))}}
 if format=="rgba16Float" {return bytes.withUnsafeBytes{Array($0.bindMemory(to:UInt16.self))}.map{Float(Float16(bitPattern:$0))}}
 return bytes.map{Float($0)/255}
}
emit(["device":device.name,"os":ProcessInfo.processInfo.operatingSystemVersionString,"fastMath":false,"sourcePin":"48e18f71655d0f0d86144c63239331833501fda2","comparison":"original independently preprocessed HLSL5→MSL vs production MSL"])
var cases=0,failedCases=0,totalDifferingBytes=0
for (inputName,inputFormat,inputStride) in formats {
 let desc=MTLTextureDescriptor.texture2DDescriptor(pixelFormat:inputFormat,width:31,height:19,mipmapped:false);desc.storageMode = .shared;desc.usage = .shaderRead
 let input=device.makeTexture(descriptor:desc)!
 var upload:[UInt8]=[]
 if inputName=="rgba32Float" {upload=Array(inputData)}
 else if inputName=="rgba16Float" {upload=originalFloats.flatMap{value->[UInt8] in var bits=Float16(value).bitPattern.littleEndian;return withUnsafeBytes(of:&bits){Array($0)}}}
 else {upload=originalFloats.map{UInt8(($0*255).rounded().clamped(to:0...255))}}
 upload.withUnsafeBytes{input.replace(region:MTLRegionMake2D(0,0,31,19),mipmapLevel:0,withBytes:$0.baseAddress!,bytesPerRow:31*inputStride)}
 for (outputName,outputFormat,outputStride) in formats {
  let pipelines=try libraries.map{library->MTLRenderPipelineState in let d=MTLRenderPipelineDescriptor();d.vertexFunction=library.makeFunction(name:"fxaa_vertex");d.fragmentFunction=library.makeFunction(name:"fxaa_fragment");d.colorAttachments[0].pixelFormat=outputFormat;return try device.makeRenderPipelineState(descriptor:d)}
  for (qualityIndex,quality) in qualities.enumerated() {for (width,height) in [(31,19),(7,11),(53,23)] {
   var results:[[UInt8]]=[]
   for pipeline in pipelines {
    let d=MTLTextureDescriptor.texture2DDescriptor(pixelFormat:outputFormat,width:width,height:height,mipmapped:false);d.storageMode = .shared;d.usage = [.renderTarget,.shaderRead]
    let output=device.makeTexture(descriptor:d)!
    let pass=MTLRenderPassDescriptor();pass.colorAttachments[0].texture=output;pass.colorAttachments[0].loadAction = .dontCare;pass.colorAttachments[0].storeAction = .store
    let command=queue.makeCommandBuffer()!;let encoder=command.makeRenderCommandEncoder(descriptor:pass)!;encoder.setRenderPipelineState(pipeline);encoder.setFragmentTexture(input,index:0);quality.withUnsafeBytes{encoder.setFragmentBytes($0.baseAddress!,length:$0.count,index:0)};encoder.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:3);encoder.endEncoding();command.commit();command.waitUntilCompleted();if let error=command.error {throw error}
    var bytes=[UInt8](repeating:0,count:width*height*outputStride);bytes.withUnsafeMutableBytes{output.getBytes($0.baseAddress!,bytesPerRow:width*outputStride,from:MTLRegionMake2D(0,0,width,height),mipmapLevel:0)};results.append(bytes)
   }
   let byteDifferences=zip(results[0],results[1]).filter{$0 != $1}.count;let a=components(results[0],outputName),b=components(results[1],outputName);var maxError:Float=0;var nonfinite=0
   for (x,y) in zip(a,b){if x.isFinite && y.isFinite{maxError=max(maxError,abs(x-y))}else{nonfinite+=1}}
   cases+=1;totalDifferingBytes+=byteDifferences;if byteDifferences != 0 {failedCases+=1}
   emit(["input":inputName,"output":outputName,"quality":qualityIndex,"width":width,"height":height,"differentBytes":byteDifferences,"maxFiniteError":maxError,"nonfiniteSourceComponents":nonfinite])
   if inputName=="rgba32Float" && outputName=="rgba32Float" && qualityIndex==0 {
    let expectedData=try Data(contentsOf:root.appendingPathComponent("cpu-\(width)-\(height).bin"))
    let expected=expectedData.withUnsafeBytes{Array($0.bindMemory(to:Float.self))}
    var cpuMaximum:Float=0;var cpuIndex=0
    for i in a.indices {let error=abs(a[i]-expected[i]);if error>cpuMaximum {cpuMaximum=error;cpuIndex=i}}
    emit(["comparison":"independent original GPU vs original C++ CPU diagnostics only", "width":width,"height":height,"maxFiniteError":cpuMaximum,"componentIndex":cpuIndex,"originalGPU":a[cpuIndex],"originalCPU":expected[cpuIndex]])
   }
  }}
 }
}
emit(["cases":cases,"casesWithDifferentBytes":failedCases,"totalDifferentBytes":totalDifferingBytes])
if failedCases != 0 {exit(1)}
extension Comparable {func clamped(to range:ClosedRange<Self>)->Self{min(range.upperBound,max(range.lowerBound,self))}}
