"""Builds a wheel of this client with the staged host inside (gpui_bridge/bin/).

The host is compiled once for the whole repository and staged by scripts/stage-host.mjs:

    pnpm host:release
    node scripts/stage-host.mjs --to python
    python build_wheel.py

or, in one step, `pnpm wheel` from the repository root. The wheel is tagged with this
platform (py3-none-win_amd64, not py3-none-any) because it carries a native executable.
"""

import platform
import shutil
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
exe = "gpui-bridge-host.exe" if sys.platform == "win32" else "gpui-bridge-host"
bin_dir = here / "gpui_bridge" / "bin"


def plat_name() -> str:
    """The wheel tag for the platform whose host is inside the package."""
    machine = platform.machine().lower()
    arm = machine in {"arm64", "aarch64"}
    if sys.platform == "win32":
        return "win_arm64" if arm else "win_amd64"
    if sys.platform == "darwin":
        return f"macosx_11_0_{'arm64' if arm else 'x86_64'}"
    return f"manylinux2014_{'aarch64' if arm else 'x86_64'}"


staged = bin_dir / exe
if not staged.exists():
    sys.exit(
        f"host not staged at {staged}\n"
        "build it and stage it first: `pnpm host:release` then "
        "`node scripts/stage-host.mjs --to python` (or `pnpm wheel` at the root)."
    )

try:
    subprocess.run(
        [
            sys.executable,
            "-m",
            "pip",
            "wheel",
            str(here),
            "--no-deps",
            # bdist_wheel's --plat-name, passed through PEP 517: a pure-Python wheel with a
            # native executable inside is py3-none-<platform>, never py3-none-any.
            f"--config-settings=--build-option=--plat-name={plat_name()}",
            "-w",
            str(here / "dist"),
        ],
        check=True,
    )
finally:
    # Without bin/, this checkout keeps using the debug build.
    shutil.rmtree(bin_dir, ignore_errors=True)
    shutil.rmtree(here / "build", ignore_errors=True)
    for info in here.glob("*.egg-info"):
        shutil.rmtree(info)
