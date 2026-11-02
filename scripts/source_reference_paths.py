"""Portable dependency locations for the pinned audio reference oracles."""
import argparse
import os
import shutil
from pathlib import Path


def arguments():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser()
    parser.add_argument('--native-root', type=Path, default=root)
    parser.add_argument('--grooveauthor-root', type=Path,
                        default=Path(os.environ.get('GROOVEAUTHOR_ROOT', root.parent / 'grooveauthor')))
    parser.add_argument('--fumen-root', type=Path,
                        default=Path(os.environ['FUMEN_ROOT']) if 'FUMEN_ROOT' in os.environ else None)
    parser.add_argument('--fumen-dll', type=Path,
                        default=Path(os.environ['FUMEN_DLL']) if 'FUMEN_DLL' in os.environ else None)
    parser.add_argument('--dotnet', default=os.environ.get('DOTNET', shutil.which('dotnet')))
    options = parser.parse_args()
    options.fumen_root = options.fumen_root or options.grooveauthor_root / 'Fumen'
    options.fumen_dll = options.fumen_dll or options.fumen_root / 'Fumen/bin/Release/net10.0/Fumen.dll'
    for name, path in [('GrooveAuthor checkout', options.grooveauthor_root / '.git'),
                       ('Fumen assembly', options.fumen_dll)]:
        if not path.exists():
            parser.error(f'{name} not found: {path}; supply the corresponding --*-root/--fumen-dll option')
    if not options.dotnet:
        parser.error('dotnet was not found; install the .NET SDK or pass --dotnet')
    return options
