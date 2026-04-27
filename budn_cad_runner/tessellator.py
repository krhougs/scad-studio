import cadquery as cq
from OCP.BRep import BRep_Tool
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.TopAbs import TopAbs_FACE
from OCP.TopExp import TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS

from .errors import RunnerError


def ref_from_refs(refs: dict, fallback: str) -> tuple[str, str]:
    for key in ("part", "component", "assembly"):
        if key in refs:
            return f"@{key}[{refs[key]}]", key
    return f"@part[{fallback}]", "part"


def shape_from_object(cq_object):
    if hasattr(cq_object, "val"):
        return cq_object.val().wrapped
    if hasattr(cq_object, "wrapped"):
        return cq_object.wrapped
    raise RunnerError(f"unsupported CadQuery object: {type(cq_object).__name__}")


def point_xyz(point, loc):
    if loc is not None and not loc.IsIdentity():
        point = point.Transformed(loc.Transformation())
    return [point.X(), point.Y(), point.Z()]


def face_normal(face):
    normal = cq.Face(face).normalAt()
    return [normal.x, normal.y, normal.z]


def face_triangles(triangulation, loc):
    positions = []
    for index in range(1, triangulation.NbTriangles() + 1):
        for node_index in triangulation.Triangle(index).Get():
            positions.extend(point_xyz(triangulation.Node(node_index), loc))
    return positions


def tessellate_shape(shape) -> dict:
    BRepMesh_IncrementalMesh(shape, 0.1, False, 0.5, True)
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    faces = []
    face_idx = 0
    while explorer.More():
        face = TopoDS.Face_s(explorer.Current())
        loc = TopLoc_Location()
        triangulation = BRep_Tool.Triangulation_s(face, loc)
        if triangulation is not None:
            normal = face_normal(face)
            positions = face_triangles(triangulation, loc)
            vertex_count = len(positions) // 3
            faces.append(
                {
                    "face_idx": face_idx,
                    "positions": positions,
                    "normals": normal * vertex_count,
                    "normal": normal,
                    "features": [],
                    "ambiguous": False,
                    "candidate_selectors": [],
                }
            )
        face_idx += 1
        explorer.Next()
    return {"faces": faces, "edges": [], "vertices": []}


def feature_map(refs: dict) -> dict:
    features = refs.get("features", {})
    return {
        name: {
            "face_indices": [],
            **{key: value for key, value in definition.items() if key in {"selector", "tag"}},
        }
        for name, definition in features.items()
    }


def transform_matrix(location) -> list[float]:
    trsf = location.wrapped.Transformation()
    return [
        trsf.Value(1, 1),
        trsf.Value(1, 2),
        trsf.Value(1, 3),
        trsf.Value(1, 4),
        trsf.Value(2, 1),
        trsf.Value(2, 2),
        trsf.Value(2, 3),
        trsf.Value(2, 4),
        trsf.Value(3, 1),
        trsf.Value(3, 2),
        trsf.Value(3, 3),
        trsf.Value(3, 4),
        0.0,
        0.0,
        0.0,
        1.0,
    ]


def part_payload(name, cq_object, refs, instance_path=None, transform=None):
    ref_text, object_kind = ref_from_refs(refs, name)
    return {
        "name": name,
        "object_kind": object_kind,
        "ref_text": ref_text,
        "instance_path": instance_path,
        "transform": transform,
        "refs": refs,
        "mesh": tessellate_shape(shape_from_object(cq_object)),
        "feature_map": feature_map(refs),
    }


def child_refs(child, name):
    metadata = getattr(child, "metadata", {}) or {}
    object_kind = metadata.get("object_kind", "part")
    ref_text = metadata.get("ref_text", f"@{object_kind}[{name}]")
    ref_id = ref_text.split("[", 1)[1].rstrip("]") if "[" in ref_text else name
    return {object_kind: ref_id, "features": {}}


def tessellate_assembly(assembly, root_ref: str) -> list[dict]:
    parts = []
    root_name = root_ref.split("[", 1)[1].rstrip("]")
    for name, child in assembly.objects.items():
        if getattr(child, "obj", None) is None:
            continue
        refs = child_refs(child, name)
        parts.append(
            part_payload(
                name,
                child.obj,
                refs,
                instance_path=f"{root_name}/{name}",
                transform=transform_matrix(child.loc),
            )
        )
    return parts


def tessellate(cq_object, refs: dict, fallback_name: str) -> tuple[str, str, list[dict]]:
    root_ref, root_kind = ref_from_refs(refs, fallback_name)
    if isinstance(cq_object, cq.Assembly):
        return root_ref, root_kind, tessellate_assembly(cq_object, root_ref)
    return root_ref, root_kind, [part_payload(fallback_name, cq_object, refs)]
