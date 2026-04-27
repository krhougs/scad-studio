import importlib.util
import sys
from pathlib import Path

from .errors import RunnerError


def resolve_script_path(project_root: Path, script: str) -> Path:
    root = project_root.resolve()
    script_path = (root / script).resolve()
    if not script_path.is_file():
        raise RunnerError(f"script not found: {script}")
    if not script_path.is_relative_to(root):
        raise RunnerError(f"script escapes project root: {script}")
    return script_path


def load(project_root: Path, script: str):
    script_path = resolve_script_path(project_root, script)
    root_text = str(project_root.resolve())
    if root_text not in sys.path:
        sys.path.insert(0, root_text)

    module_name = f"budn_project.{script_path.stem}_{abs(hash(script_path))}"
    spec = importlib.util.spec_from_file_location(module_name, script_path)
    if spec is None or spec.loader is None:
        raise RunnerError(f"cannot load script: {script}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    refs = getattr(module, "REFS", {})
    build_fn = getattr(module, "build", None)
    if build_fn is None or not callable(build_fn):
        raise RunnerError("module has no build() function")
    return module, refs, build_fn, script_path
