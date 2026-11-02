#!/usr/bin/env python3
"""Execute the unchanged pinned GrooveAuthor timing recorder, without its UI."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=pathlib.Path, required=True)
    parser.add_argument('--dotnet', required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    revision = subprocess.check_output(['git', '-C', str(args.source), 'rev-parse', 'HEAD'], text=True).strip()
    pinned = '48e18f71655d0f0d86144c63239331833501fda2'
    if revision != pinned:
        raise SystemExit(f'Expected {pinned}; found {revision}')
    source_bytes = subprocess.check_output(['git', '-C', str(args.source), 'show',
        pinned + ':StepManiaEditor/PerformanceMonitor.cs'])
    cases = []
    def case(name, capacity, operations):
        cases.append(dict(name=name, capacity=capacity, operations=operations))
    case('empty', 3, [])
    case('one_incomplete', 3, [['begin', 100]])
    case('completed_and_accumulated', 3, [['begin',100], ['add',7], ['add',11], ['begin',130]])
    case('ring_wrap', 3, [['begin',x] for x in [100,110,130,160,200,250]])
    case('paused_closes_current', 3, [['begin',100], ['add',7], ['enabled',False], ['add',99], ['begin',130], ['begin',200]])
    case('resume_skips_paused_frames', 3, [['begin',100], ['begin',120], ['enabled',False], ['begin',150], ['enabled',True], ['begin',180], ['add',9], ['begin',210]])
    case('zero_length_frame', 3, [['begin',100], ['begin',100], ['begin',110]])
    case('no_current_timing_is_ignored', 3, [['add',17], ['begin',100], ['begin',110]])
    case('capacity_one', 1, [['begin',100], ['begin',110], ['begin',120]])
    case('negative_and_zero_counter', 3, [['begin',100], ['add',7], ['add',-7], ['begin',110]])
    program = r'''
using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using StepManiaEditor;
class Program {
 static void Main(string[] args) {
  using var document=JsonDocument.Parse(File.ReadAllText(args[0]));
  var results=new List<object>();
  foreach(var test in document.RootElement.EnumerateArray()) {
   var monitor=new PerformanceMonitor(test.GetProperty("capacity").GetInt32(),new[]{"Work"});
   var snapshots=new List<object>();
   foreach(var operation in test.GetProperty("operations").EnumerateArray()) {
    switch(operation[0].GetString()) {
     case "begin":monitor.BeginFrameFromStopWatch(operation[1].GetInt64());break;
     case "add":monitor.SetTimeFromStopWatch("Work",operation[1].GetInt64());break;
     case "enabled":monitor.SetEnabled(operation[1].GetBoolean());break;
    }
    snapshots.Add(Capture(monitor));
   }
   results.Add(new{name=test.GetProperty("name").GetString(),snapshots});
  }
  Console.WriteLine(JsonSerializer.Serialize(results));
 }
 static object Capture(PerformanceMonitor monitor) {
  var frames=new List<long[]>();
  foreach(var frame in monitor) {
   var timings=new List<long>();
   foreach(var timing in frame.GetTimingData())timings.Add(timing.GetTicks());
   frames.Add(timings.ToArray());
  }
  return frames;
 }
}
'''
    with tempfile.TemporaryDirectory(prefix='ga-performance-reference-') as temp:
        root = pathlib.Path(temp)
        (root/'Reference.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><EnableDefaultCompileItems>true</EnableDefaultCompileItems></PropertyGroup></Project>')
        (root/'PerformanceMonitor.cs').write_bytes(source_bytes)
        (root/'Program.cs').write_text(program)
        (root/'cases.json').write_text(json.dumps(cases))
        subprocess.run([args.dotnet,'build',str(root/'Reference.csproj'),'-c','Release','--nologo','--verbosity','quiet'],check=True)
        output=subprocess.check_output([args.dotnet,str(root/'bin/Release/net10.0/Reference.dll'),str(root/'cases.json')],text=True)
        results=json.loads(output)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(dict(revision=revision, source_sha256=hashlib.sha256(source_bytes).hexdigest(), scope='Original recorder with deterministic BeginFrame/SetTime ticks; real stopwatch Start/End and UI not executed', cases=cases, results=results), indent=2)+'\n')


if __name__ == '__main__':
    main()
