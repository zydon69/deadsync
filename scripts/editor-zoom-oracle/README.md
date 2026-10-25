# Zoom reference runner

Development-only runner for the unchanged `ZoomManager.cs` Git object at
GrooveAuthor `48e18f71655d0f0d86144c63239331833501fda2`. The generator copies that
object into ignored `obj/`; it is never linked into DeadSync.

The runner references an already built .NET 10 Fumen assembly in the pinned
StepManiaLibrary checkout. `Adapters.cs` supplies only the preference singleton,
explicit observer notifications, spacing-mode enum and held-input flags required
by the original manager. Snapshots read the manager's internal values, including
spacing values, rather than treating the adapter as proof of the original
preference setters, persistence, full editor or hardware input handling.

Generate expected traces from the committed JSON requests:

```sh
python3 scripts/editor-generate-zoom-fixtures.py \
  --editor-root /path/to/GrooveAuthor \
  --reference-root /path/to/StepManiaLibrary \
  --dotnet /path/to/dotnet
```

The script checks the library/submodule pins and records the original source and
Fumen assembly hashes. The library checkout must already contain
`Fumen/Fumen/bin/Release/net10.0/Fumen.dll` (the existing editor fixture workflow
prepares this development reference). .NET is not needed for ordinary Rust tests:

```sh
cargo test -p deadsync-editor --test zoom_workflows --jobs 2 --locked
```

The committed expected snapshots cover valid finite inputs. Rust additionally
rejects nonfinite input and preference values outside the original UI ranges.
