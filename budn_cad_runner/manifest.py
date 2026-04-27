import ast
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

from . import __version__


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_json(value) -> str:
    data = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return sha256_bytes(data)


def file_hash(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def module_candidates(module_name: str) -> list[Path]:
    parts = module_name.split(".")
    return [Path(*parts).with_suffix(".py"), Path(*parts) / "__init__.py"]


def imported_modules(source: str) -> set[str]:
    tree = ast.parse(source)
    modules = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            modules.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            modules.add(node.module)
    return modules


def find_project_imports(project_root: Path, script_path: Path) -> list[Path]:
    root = project_root.resolve()
    pending = [script_path.resolve()]
    seen = set()
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        for module in imported_modules(path.read_text()):
            for candidate in module_candidates(module):
                import_path = (root / candidate).resolve()
                if import_path.is_file() and import_path not in seen:
                    pending.append(import_path)
    return sorted(seen, key=lambda item: item.relative_to(root).as_posix())


def build_manifest(project_root: Path, script_path: Path, params: dict, exports: dict) -> dict:
    root = project_root.resolve()
    source_hash = file_hash(script_path)
    dependencies = [
        {"path": path.relative_to(root).as_posix(), "hash": file_hash(path)}
        for path in find_project_imports(root, script_path)
    ]
    params_hash = sha256_json(params)
    deps_hash = sha256_json(dependencies)
    return {
        "source_path": script_path.relative_to(root).as_posix(),
        "source_hash": source_hash,
        "params": params,
        "params_hash": params_hash,
        "dependencies": dependencies,
        "deps_hash": deps_hash,
        "runner_version": __version__,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "export_hashes": {name: file_hash(Path(path)) for name, path in exports.items()},
    }


def build_id(manifest: dict) -> str:
    return sha256_json(
        {
            "source_hash": manifest["source_hash"],
            "params_hash": manifest["params_hash"],
            "deps_hash": manifest["deps_hash"],
        }
    )
