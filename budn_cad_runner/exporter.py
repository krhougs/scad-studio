from pathlib import Path

from .errors import RunnerError

EXPORT_TYPES = {
    "step": ("step", "STEP"),
    "stl": ("stl", "STL"),
    "3mf": ("3mf", "3MF"),
}


def parse_export_formats(raw: str) -> list[str]:
    formats = []
    for item in raw.split(","):
        value = item.strip().lower()
        if value:
            formats.append(value)
    return formats


def export_artifacts(
    cq_object,
    project_root: Path,
    output_dir: Path,
    formats: list[str],
    stem: str,
) -> dict:
    project_root = project_root.resolve()
    exports = []
    for export_format in formats:
        extension, export_type = export_type_for(export_format)
        path = output_dir / f"{stem}.{extension}"
        exports.append((export_format, export_type, path, display_path(project_root, path)))
    if exports:
        output_dir.mkdir(parents=True, exist_ok=True)
    exported = {}
    for export_format, export_type, path, display in exports:
        cadquery_export(cq_object, path, export_type)
        exported[export_format] = display
    return exported


def export_type_for(export_format: str) -> tuple[str, str]:
    if export_format not in EXPORT_TYPES:
        raise RunnerError(f"unsupported export format: {export_format}")
    return EXPORT_TYPES[export_format]


def display_path(project_root: Path, path: Path) -> str:
    resolved = path.resolve()
    if resolved.is_relative_to(project_root):
        return resolved.relative_to(project_root).as_posix()
    raise RunnerError("export path must stay inside project root")


def cadquery_export(cq_object, path: Path, export_type: str):
    from cadquery import exporters

    exporters.export(cq_object, str(path), exportType=export_type)
