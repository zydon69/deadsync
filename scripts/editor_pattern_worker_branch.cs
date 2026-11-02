            if(request.TryGetProperty("ActualWorker",out _)) {
                var json=new JsonSerializerOptions{IncludeFields=true,Converters={new System.Text.Json.Serialization.JsonStringEnumConverter()}};
                var performed=JsonSerializer.Deserialize<StepManiaLibrary.PerformedChart.Config>(request.GetProperty("PerformedConfig"),json)!;performed.Init();
                foreach(var marker in markers){marker.Config=performed;marker.IgnorePrecedingDistribution=request.TryGetProperty("IgnoreRows",out var ignoredRows)&&ignoredRows.EnumerateArray().Any(r=>r.GetInt32()==marker.GetRow());marker.RandomSeed=request.TryGetProperty("WorkerSeed",out var workerSeed)?workerSeed.GetInt32():42+marker.GetRow()%11;marker.PatternConfig=JsonSerializer.Deserialize<StepManiaLibrary.PerformedChart.PatternConfig>(request.GetProperty("PatternConfig"),json)!;marker.PatternConfig.BeatSubDivision=48/marker.GetStepSpacing();marker.PatternConfig.Init();}
                StepManiaEditor.AutogenConfig.ExpressedChartConfigManager.Instance.Value=new StepManiaLibrary.ExpressedChart.Config{BracketParsingDetermination=StepManiaLibrary.ExpressedChart.BracketParsingDetermination.UseDefaultMethod,DefaultBracketParsingMethod=request.TryGetProperty("WorkerExpressionBalanced",out _)?StepManiaLibrary.ExpressedChart.BracketParsingMethod.Balanced:StepManiaLibrary.ExpressedChart.BracketParsingMethod.NoBrackets};
                StepManiaEditor.AutogenConfig.ExpressedChartConfigManager.Instance.Value.Init();
                var worker=new ActionAutoGeneratePatterns(new Editor(referenceRoot),chart,markers);
                var before=new NotesOnlyWriter().Run(chart);worker.Run();worker.Completion.Task.Wait(TimeSpan.FromSeconds(60));
                if(!worker.Completion.Task.IsCompleted)throw new Exception("Worker timeout");
                var after=new NotesOnlyWriter().Run(chart);worker.Undo();var undo=new NotesOnlyWriter().Run(chart);worker.Run();var redo=new NotesOnlyWriter().Run(chart);
                schedule=new{Before=before,NoteData=after,Undo=undo,Redo=redo,Logs=Logger.Messages};
            }
