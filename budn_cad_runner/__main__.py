import argparse
import json
import sys
import traceback
from pathlib import Path
from typing import Optional

from .errors import BuildError
from .executor import run
from .loader import load
from .manifest import build_manifest


def parse_params(raw: str) -> dict:
    if not raw:
        return {}
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError("--params must be a JSON object")
    return value


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="budn_cad_runner")
    parser.add_argument("--script", required=True)
    parser.add_argument("--project-root", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--exports", default="")
    parser.add_argument("--params", default="{}")
    return parser.parse_args(argv)


def execute(args: argparse.Namespace) -> dict:
    from . import schema

    project_root = Path(args.project_root)
    params = parse_params(args.params)
    _, refs, build_fn, script_path = load(project_root, args.script)
    cq_object = run(build_fn, params)
    exports = {}
    manifest = build_manifest(project_root, script_path, params, exports)
    return schema.success(cq_object, refs, script_path, manifest, exports)


def emit(payload: dict):
    print(json.dumps(payload, ensure_ascii=False, separators=(",", ":")))


def error_payload(status: str, error_type: str, message: str) -> dict:
    return {
        "status": status,
        "error": message,
        "error_type": error_type,
        "result_id": None,
        "build_id": None,
        "unit": "millimeter",
        "parts": [],
        "exports": {},
        "metadata": None,
        "manifest": None,
    }


def print_traceback(error: BaseException):
    traceback.print_exception(type(error), error, error.__traceback__, file=sys.stderr)


def main(argv: Optional[list[str]] = None) -> int:
    try:
        args = parse_args(sys.argv[1:] if argv is None else argv)
        emit(execute(args))
        return 0
    except BuildError as error:
        print_traceback(error.original)
        emit(error_payload("build_error", type(error.original).__name__, str(error.original)))
        return 1
    except Exception as error:
        print_traceback(error)
        emit(error_payload("runner_error", type(error).__name__, str(error)))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
