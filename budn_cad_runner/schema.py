from .manifest import build_id
from .metadata import compute
from .tessellator import tessellate


def success(cq_object, refs, script_path, manifest, exports) -> dict:
    fallback_name = script_path.stem
    root_ref, root_kind, parts = tessellate(cq_object, refs, fallback_name)
    return {
        "status": "success",
        "error": None,
        "error_type": None,
        "result_id": "cq_" + build_id(manifest).removeprefix("sha256:")[:16],
        "build_id": build_id(manifest),
        "unit": "millimeter",
        "root_ref_text": root_ref,
        "root_object_kind": root_kind,
        "parts": parts,
        "exports": exports,
        "metadata": compute(cq_object),
        "manifest": manifest,
    }


def error(status: str, error_type: str, message: str) -> dict:
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
