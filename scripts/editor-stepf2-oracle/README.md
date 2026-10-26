# StepF2 reader / writer reference

Development-only executable against the pinned Fumen assembly. It is not shipped
with DeadSync. It executes actual SM/SSC readers and writers, projecting note
owners, kinds, hold endpoints and source flags; it does not run GrooveAuthor UI.

Using the same prepared .NET 10 reference checkout as the main editor oracle:

```sh
/tmp/deadsync-dotnet10/dotnet build scripts/editor-stepf2-oracle/StepF2Oracle.csproj -c Release -p:ReferenceRoot=/tmp/deadsync-grooveauthor-step-library
/tmp/deadsync-dotnet10/dotnet scripts/editor-stepf2-oracle/bin/Release/net10.0/StepF2Oracle.dll crates/deadsync-editor/tests/fixtures/grooveauthor/stepf2-notes.request.json
```

The JSON contains 24 cases (SM/SSC, Pump Double/Routine, six note streams).
`stepf2-notes.reference.json` binds the input, output, projector, reference source
hashes and assembly hash. A writer drops visibility and fake-hold source flags;
Rust preserves those attributes. Fake-hold flags remain decorations and do not
change a hold's note kind. The normal non-coop Pump Routine minimum participant
count differs from the existing DeadSync raw-grid contract and is not claimed
by these differential checks. Actual co-op owner counts are checked exactly.
