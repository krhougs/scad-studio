import cadquery as cq
from OCP.BRep import BRep_Tool
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE, TopAbs_VERTEX
from OCP.TopExp import TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS

from .errors import RunnerError
from .ref_mapper import map_features


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


def vector_xyz(vector):
    return [vector.x, vector.y, vector.z]


def vertex_xyz(vertex):
    point = BRep_Tool.Pnt_s(vertex)
    return [point.X(), point.Y(), point.Z()]


def append_unique(shapes: list, candidate) -> int:
    for index, shape in enumerate(shapes):
        if candidate.IsSame(shape):
            return index
    shapes.append(candidate)
    return len(shapes) - 1


def collect_shapes(shape, kind, caster) -> list:
    explorer = TopExp_Explorer(shape, kind)
    shapes = []
    while explorer.More():
        append_unique(shapes, caster(explorer.Current()))
        explorer.Next()
    return shapes


def edge_vertices(edge) -> list:
    return collect_shapes(edge, TopAbs_VERTEX, TopoDS.Vertex_s)


def face_edges(face) -> list:
    return collect_shapes(face, TopAbs_EDGE, TopoDS.Edge_s)


def edge_polyline(edge) -> list:
    points = []
    try:
        sampled, _ = cq.Edge(edge).sample(12)
        points = [vector_xyz(point) for point in sampled]
    except Exception:
        points = [vertex_xyz(vertex) for vertex in edge_vertices(edge)]
    if len(points) < 2:
        points = [vertex_xyz(vertex) for vertex in edge_vertices(edge)]
    return [coordinate for point in points for coordinate in point]


def edge_payloads(edges: list, topo_faces: list) -> list[dict]:
    payloads = []
    face_edge_sets = [face_edges(face) for face in topo_faces]
    for edge_idx, edge in enumerate(edges):
        adjacent_faces = []
        for face_idx, face_edges_for_face in enumerate(face_edge_sets):
            if any(edge.IsSame(face_edge) for face_edge in face_edges_for_face):
                adjacent_faces.append(face_idx)
        payloads.append(
            {
                "edge_idx": edge_idx,
                "polyline": edge_polyline(edge),
                "adjacent_faces": adjacent_faces,
            }
        )
    return payloads


def vertex_payloads(vertices: list, edges: list) -> list[dict]:
    payloads = []
    edge_vertex_sets = [edge_vertices(edge) for edge in edges]
    for vertex_idx, vertex in enumerate(vertices):
        adjacent_edges = []
        for edge_idx, vertices_for_edge in enumerate(edge_vertex_sets):
            if any(vertex.IsSame(edge_vertex) for edge_vertex in vertices_for_edge):
                adjacent_edges.append(edge_idx)
        payloads.append(
            {
                "vertex_idx": vertex_idx,
                "position": vertex_xyz(vertex),
                "adjacent_edges": adjacent_edges,
            }
        )
    return payloads


def tessellate_shape(shape) -> tuple[dict, list]:
    BRepMesh_IncrementalMesh(shape, 0.1, False, 0.5, True)
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    faces = []
    topo_faces = []
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
            topo_faces.append(face)
        face_idx += 1
        explorer.Next()
    edges = collect_shapes(shape, TopAbs_EDGE, TopoDS.Edge_s)
    vertices = collect_shapes(shape, TopAbs_VERTEX, TopoDS.Vertex_s)
    return {
        "faces": faces,
        "edges": edge_payloads(edges, topo_faces),
        "vertices": vertex_payloads(vertices, edges),
    }, topo_faces


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


def composed_child_location(assembly, name: str, child):
    location = child.loc
    path_segments = name.split("/")
    for index in range(1, len(path_segments)):
        parent_name = "/".join(path_segments[:index])
        parent = assembly.objects.get(parent_name)
        if parent is not None and getattr(parent, "obj", None) is None:
            location = parent.loc * location
    return location


def part_payload(name, cq_object, refs, instance_path=None, transform=None):
    ref_text, object_kind = ref_from_refs(refs, name)
    mesh, topo_faces = tessellate_shape(shape_from_object(cq_object))
    return {
        "name": name,
        "object_kind": object_kind,
        "ref_text": ref_text,
        "instance_path": instance_path,
        "transform": transform,
        "refs": refs,
        "mesh": mesh,
        "feature_map": map_features(cq_object, refs, mesh, topo_faces),
    }


def child_refs(child, name):
    metadata = getattr(child, "metadata", {}) or {}
    object_kind = metadata.get("object_kind", "part")
    ref_text = metadata.get("ref_text", f"@{object_kind}[{name}]")
    ref_id = ref_text.split("[", 1)[1].rstrip("]") if "[" in ref_text else name
    refs = metadata.get("refs", {}) if isinstance(metadata.get("refs"), dict) else {}
    features = metadata.get("features", refs.get("features", {}))
    if not isinstance(features, dict):
        features = {}
    return {object_kind: ref_id, "features": features}


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
                transform=transform_matrix(composed_child_location(assembly, name, child)),
            )
        )
    return parts


def tessellate(cq_object, refs: dict, fallback_name: str) -> tuple[str, str, list[dict]]:
    root_ref, root_kind = ref_from_refs(refs, fallback_name)
    if isinstance(cq_object, cq.Assembly):
        return root_ref, root_kind, tessellate_assembly(cq_object, root_ref)
    return root_ref, root_kind, [part_payload(fallback_name, cq_object, refs)]
