#!/usr/bin/env python3
"""Pin headless regression images. Usage: snap-ui.py OUT [BIN_DIR]. No native windows."""
import os
import pathlib
import subprocess
import sys

out = pathlib.Path(sys.argv[1])
bin_dir = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path('target/release')
out.mkdir(parents=True, exist_ok=True)
cases = {
    'settings-dark': ('settings', ['--scene', 'dark']),
    'settings-light': ('settings', ['--scene', 'light']),
}
for name, (binary, args) in cases.items():
    # A release build reads Rune's flags only with RUNE_DEV=1.
    subprocess.run([str(bin_dir / binary), *args, '--snap', str(out / f'{name}.png')], check=True,
                   env=dict(os.environ, RUNE_DEV='1'))
