#!/usr/bin/env python3

from __future__ import annotations

import shutil
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import zipfile
from argparse import ArgumentParser
from pathlib import Path


REQUIRED_PACKAGE_FILES = {
    "stateset_embedded/__init__.py",
    "stateset_embedded/__init__.pyi",
    "stateset_embedded/agent_toolkit.py",
    "stateset_embedded/autogen.py",
    "stateset_embedded/crewai.py",
    "stateset_embedded/generic.py",
    "stateset_embedded/langchain.py",
    "stateset_embedded/openai.py",
    "stateset_embedded/py.typed",
}

REQUIRED_SDIST_FILES = {
    "pyproject.toml",
    "README.md",
    "python/stateset_embedded/__init__.py",
    "python/stateset_embedded/__init__.pyi",
    "python/stateset_embedded/agent_toolkit.py",
    "python/stateset_embedded/autogen.py",
    "python/stateset_embedded/crewai.py",
    "python/stateset_embedded/generic.py",
    "python/stateset_embedded/langchain.py",
    "python/stateset_embedded/openai.py",
    "python/stateset_embedded/py.typed",
}

REQUIRED_EXTRAS = {"langchain", "crewai", "autogen", "agents"}

IMPORT_SMOKE = textwrap.dedent(
    """
    from stateset_embedded import (
        Commerce,
        create_autogen_tools,
        create_callable_registry,
        create_crewai_tools,
        create_embedded_agent_toolkit,
        create_langchain_tools,
        create_openai_tools,
        create_tool_descriptors,
        execute_openai_tool_call,
        execute_tool,
    )
    from stateset_embedded.autogen import create_autogen_tools as module_create_autogen_tools
    from stateset_embedded.crewai import create_crewai_tools as module_create_crewai_tools
    from stateset_embedded.generic import (
        create_callable_registry as module_create_callable_registry,
        create_tool_descriptors as module_create_tool_descriptors,
        execute_tool as module_execute_tool,
    )
    from stateset_embedded.langchain import create_langchain_tools as module_create_langchain_tools
    from stateset_embedded.openai import (
        create_openai_tools as module_create_openai_tools,
        execute_openai_tool_call as module_execute_openai_tool_call,
    )

    commerce = Commerce(":memory:")
    toolkit = create_embedded_agent_toolkit(commerce, allow_apply=False)

    openai_tools = create_openai_tools(commerce, filter=["list_customers"])
    assert openai_tools[0]["function"]["name"] == "list_customers"
    assert module_create_openai_tools(commerce, filter=["list_customers"])[0]["function"]["name"] == "list_customers"

    descriptors = create_tool_descriptors(commerce, filter=["list_customers"])
    assert descriptors[0].name == "list_customers"
    assert module_create_tool_descriptors(commerce, filter=["list_customers"])[0].name == "list_customers"

    registry = create_callable_registry(commerce, filter=["list_customers"])
    assert "list_customers" in registry
    assert registry["list_customers"]({})["success"] is True
    assert module_create_callable_registry(commerce, filter=["list_customers"])["list_customers"]({})["success"] is True

    assert execute_tool(commerce, "list_customers")["success"] is True
    assert module_execute_tool(commerce, "list_customers")["success"] is True

    tool_call = {"call_id": "pkg_smoke_1", "function": {"name": "list_customers", "arguments": "{}"}}
    execution = execute_openai_tool_call(commerce, tool_call)
    module_execution = module_execute_openai_tool_call(commerce, tool_call)
    assert execution["name"] == "list_customers"
    assert execution["result"]["tool"] == "list_customers"
    assert execution["output_message"]["call_id"] == "pkg_smoke_1"
    assert module_execution["name"] == "list_customers"
    assert module_execution["result"]["tool"] == "list_customers"
    assert module_execution["output_message"]["call_id"] == "pkg_smoke_1"

    adapter_factory = lambda descriptor: descriptor.name
    assert create_langchain_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]
    assert module_create_langchain_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]
    assert create_crewai_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]
    assert module_create_crewai_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]
    assert create_autogen_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]
    assert module_create_autogen_tools(commerce, filter=["list_customers"], tool_factory=adapter_factory) == ["list_customers"]

    assert toolkit.get_tool("list_customers", format="openai")["function"]["name"] == "list_customers"
    """
)


def run(command: list[str], cwd: Path | None = None) -> None:
    subprocess.run(command, cwd=cwd, check=True)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def normalize_sdist_names(names: list[str]) -> set[str]:
    normalized = set()
    for name in names:
        parts = Path(name).parts
        if len(parts) <= 1:
            continue
        normalized.add(str(Path(*parts[1:])))
    return normalized


def parse_args() -> tuple[Path, str, Path | None]:
    parser = ArgumentParser(
        description="Validate the Python binding wheel/sdist contents and import surface.",
    )
    parser.add_argument("bindings_python_dir")
    parser.add_argument("python_bin")
    parser.add_argument(
        "--dist-dir",
        dest="dist_dir",
        help="Validate prebuilt artifacts from this directory instead of building new ones.",
    )
    args = parser.parse_args()
    return Path(args.bindings_python_dir).resolve(), args.python_bin, (
        Path(args.dist_dir).resolve() if args.dist_dir else None
    )


def validate_wheel(wheel_path: Path) -> None:
    with zipfile.ZipFile(wheel_path) as wheel_file:
        wheel_names = set(wheel_file.namelist())
        missing_wheel_files = sorted(REQUIRED_PACKAGE_FILES - wheel_names)
        require(not missing_wheel_files, f"Wheel {wheel_path.name} is missing required package files: {missing_wheel_files}")

        native_entries = [
            name
            for name in wheel_names
            if name.startswith("stateset_embedded/")
            and (name.endswith(".so") or name.endswith(".pyd") or name.endswith(".dylib"))
        ]
        require(native_entries, f"Wheel {wheel_path.name} is missing the native extension module.")

        metadata_name = next(
            (name for name in wheel_names if name.endswith(".dist-info/METADATA")),
            None,
        )
        require(metadata_name is not None, f"Wheel {wheel_path.name} is missing METADATA.")

        metadata = wheel_file.read(metadata_name).decode("utf8")
        missing_extras = sorted(
            extra for extra in REQUIRED_EXTRAS if f"Provides-Extra: {extra}" not in metadata
        )
        require(not missing_extras, f"Wheel {wheel_path.name} metadata is missing extras: {missing_extras}")


def validate_sdist(sdist_path: Path) -> None:
    with tarfile.open(sdist_path, "r:gz") as sdist_file:
        normalized_names = normalize_sdist_names(sdist_file.getnames())
        missing_sdist_files = sorted(REQUIRED_SDIST_FILES - normalized_names)
        require(
            not missing_sdist_files,
            f"Source distribution {sdist_path.name} is missing required files: {missing_sdist_files}",
        )


def build_dist(bindings_dir: Path, python_bin: str, dist_dir: Path) -> None:
    run([python_bin, "-m", "maturin", "build", "--interpreter", python_bin, "--out", str(dist_dir)], cwd=bindings_dir)
    run([python_bin, "-m", "maturin", "sdist", "--out", str(dist_dir)], cwd=bindings_dir)


def smoke_install(dist_dir: Path, python_bin: str) -> None:
    venv_dir = dist_dir / "wheel-smoke-venv"
    run([python_bin, "-m", "venv", str(venv_dir)])

    venv_python = venv_dir / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
    run([
        str(venv_python),
        "-m",
        "pip",
        "install",
        "--no-index",
        "--find-links",
        str(dist_dir),
        "--no-deps",
        "stateset-embedded",
    ])
    run([str(venv_python), "-c", IMPORT_SMOKE])


def main() -> None:
    bindings_dir, python_bin_arg, dist_dir_arg = parse_args()

    python_bin = shutil.which(python_bin_arg) or python_bin_arg

    if dist_dir_arg is not None:
        dist_dir = dist_dir_arg
        require(dist_dir.is_dir(), f"Distribution directory does not exist: {dist_dir}")

        wheel_paths = sorted(dist_dir.glob("stateset_embedded-*.whl"))
        sdist_paths = sorted(dist_dir.glob("stateset_embedded-*.tar.gz"))

        require(wheel_paths, f"No wheels found in distribution directory: {dist_dir}")
        require(sdist_paths, f"No source distributions found in distribution directory: {dist_dir}")

        for wheel_path in wheel_paths:
            validate_wheel(wheel_path)
        for sdist_path in sdist_paths:
            validate_sdist(sdist_path)

        smoke_install(dist_dir, python_bin)
        wheel_names = ", ".join(path.name for path in wheel_paths)
        sdist_names = ", ".join(path.name for path in sdist_paths)
        print(
            f"Python binding package shape is valid for stateset-embedded with wheels [{wheel_names}] and sdists [{sdist_names}].",
        )
        return

    with tempfile.TemporaryDirectory(prefix="stateset-python-dist-") as temp_dir:
        dist_dir = Path(temp_dir)
        build_dist(bindings_dir, python_bin, dist_dir)

        wheel_paths = sorted(dist_dir.glob("stateset_embedded-*.whl"))
        sdist_paths = sorted(dist_dir.glob("stateset_embedded-*.tar.gz"))

        require(wheel_paths, "maturin build did not produce a wheel.")
        require(sdist_paths, "maturin sdist did not produce a source distribution.")

        for wheel_path in wheel_paths:
            validate_wheel(wheel_path)
        for sdist_path in sdist_paths:
            validate_sdist(sdist_path)

        smoke_install(dist_dir, python_bin)
        print(
            f"Python binding package shape is valid for stateset-embedded with wheel {wheel_paths[0].name} and sdist {sdist_paths[0].name}.",
        )


if __name__ == "__main__":
    main()
