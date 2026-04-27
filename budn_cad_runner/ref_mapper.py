from OCP.TopAbs import TopAbs_FACE
from OCP.TopExp import TopExp_Explorer
from OCP.TopoDS import TopoDS

from .errors import RunnerError
from .selector_parser import evaluate_selector


def map_features(cq_object, refs: dict, mesh: dict, topo_faces: list) -> dict:
    feature_defs = refs.get("features", {})
    feature_map = {}
    for name, definition in feature_defs.items():
        indices = match_feature(cq_object, definition, topo_faces)
        feature_map[name] = feature_payload(definition, indices)
        apply_feature_to_faces(mesh["faces"], name, indices)
    return feature_map


def match_feature(cq_object, definition: dict, topo_faces: list) -> list[int]:
    if "selector" in definition:
        selected = evaluate_selector(cq_object, definition["selector"])
        return match_selected_faces(selected, topo_faces)
    if "tag" in definition:
        selected = tagged_workplane(cq_object, definition["tag"])
        return match_selected_faces(selected, topo_faces)
    return []


def tagged_workplane(cq_object, tag: str):
    getter = getattr(cq_object, "_getTagged", None)
    if getter is None or not callable(getter):
        raise RunnerError(f"CadQuery object does not support tag lookup: {tag}")
    return getter(tag)


def match_selected_faces(selection, topo_faces: list) -> list[int]:
    selected_faces = selection_faces(selection)
    indices = []
    for index, topo_face in enumerate(topo_faces):
        if any(selected.IsSame(topo_face) for selected in selected_faces):
            indices.append(index)
    return indices


def selection_faces(selection) -> list:
    faces = []
    for item in selection_values(selection):
        wrapped = wrapped_shape(item)
        if wrapped.ShapeType() == TopAbs_FACE:
            faces.append(TopoDS.Face_s(wrapped))
        else:
            faces.extend(faces_from_shape(wrapped))
    return faces


def selection_values(selection) -> list:
    if hasattr(selection, "vals"):
        return selection.vals()
    if isinstance(selection, (list, tuple)):
        return list(selection)
    return [selection]


def wrapped_shape(item):
    if hasattr(item, "wrapped"):
        return item.wrapped
    if hasattr(item, "val"):
        return item.val().wrapped
    raise RunnerError(f"selected value has no wrapped shape: {type(item).__name__}")


def faces_from_shape(shape) -> list:
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    faces = []
    while explorer.More():
        faces.append(TopoDS.Face_s(explorer.Current()))
        explorer.Next()
    return faces


def feature_payload(definition: dict, indices: list[int]) -> dict:
    payload = {"face_indices": indices}
    for key in ("selector", "tag"):
        if key in definition:
            payload[key] = definition[key]
    return payload


def apply_feature_to_faces(faces: list, name: str, indices: list[int]):
    for index in indices:
        if 0 <= index < len(faces) and name not in faces[index]["features"]:
            faces[index]["features"].append(name)
