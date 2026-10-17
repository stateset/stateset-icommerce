#!/usr/bin/env python3
"""Exercise wheel acceptance with real, offline pip installs in clean venvs."""

from __future__ import annotations

import importlib.util
import io
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "python_binding_package_check",
    Path(__file__).with_name("check_python_binding_package.py"),
)
assert SPEC is not None and SPEC.loader is not None
package_check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package_check)


class WheelAcceptanceTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="stateset-wheel-acceptance-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.dist = self.root / "dist"
        self.dist.mkdir()
        self.source_build_marker = self.root / "source-build-attempted"

    def wheel(self, tag: str = "py3-none-any") -> None:
        info = "stateset_embedded-0.0.0.dist-info"
        with zipfile.ZipFile(self.dist / f"stateset_embedded-0.0.0-{tag}.whl", "w") as archive:
            archive.writestr(
                "stateset_embedded/__init__.py",
                "__packaging_wheel_fixture__ = True\n",
            )
            archive.writestr(
                f"{info}/METADATA",
                "Metadata-Version: 2.1\nName: stateset-embedded\nVersion: 0.0.0\n\n",
            )
            archive.writestr(
                f"{info}/WHEEL",
                "Wheel-Version: 1.0\nGenerator: acceptance-test\n"
                f"Root-Is-Purelib: true\nTag: {tag}\n",
            )
            archive.writestr(f"{info}/RECORD", "")

    def sdist(self) -> None:
        backend = (
            "def get_requires_for_build_wheel(config_settings=None):\n"
            "    from pathlib import Path\n"
            f"    Path({str(self.source_build_marker)!r}).write_text('source build attempted')\n"
            "    raise RuntimeError('source fallback must not satisfy wheel acceptance')\n"
        )
        files = {
            "pyproject.toml": (
                '[build-system]\nrequires = []\nbuild-backend = "backend"\nbackend-path = ["."]\n'
            ),
            "backend.py": backend,
        }
        with tarfile.open(self.dist / "stateset_embedded-0.0.0.tar.gz", "w:gz") as archive:
            for name, content in files.items():
                data = content.encode()
                member = tarfile.TarInfo(f"stateset_embedded-0.0.0/{name}")
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))

    def test_installed_wheel_is_imported_despite_source_shadow(self) -> None:
        self.wheel()
        self.sdist()
        shadow = self.root / "shadow" / "stateset_embedded"
        shadow.mkdir(parents=True)
        (shadow / "__init__.py").write_text(
            "raise RuntimeError('source tree shadowed the installed wheel')\n",
        )
        smoke = (
            "import sys, stateset_embedded as package\n"
            "from pathlib import Path\n"
            "assert package.__packaging_wheel_fixture__ is True\n"
            "assert Path(package.__file__).resolve().is_relative_to(Path(sys.prefix).resolve())\n"
        )
        with (
            mock.patch.dict(os.environ, {"PYTHONPATH": str(shadow.parent)}),
            mock.patch.object(package_check, "IMPORT_SMOKE", smoke),
        ):
            package_check.smoke_install(self.dist, sys.executable)
        self.assertFalse(self.source_build_marker.exists())

    def test_incompatible_wheel_cannot_fall_back_to_sdist(self) -> None:
        self.wheel("cp999-cp999-any")
        self.sdist()
        with self.assertRaises(subprocess.CalledProcessError) as error:
            package_check.smoke_install(self.dist, sys.executable)
        self.assertIn("install", error.exception.cmd)
        self.assertFalse(
            self.source_build_marker.exists(),
            "the installer attempted a source build instead of refusing the incompatible wheel",
        )


if __name__ == "__main__":
    unittest.main()
