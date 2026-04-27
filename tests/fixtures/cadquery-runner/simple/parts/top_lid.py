import cadquery as cq

REFS = {
    "part": "top_lid",
    "features": {
        "outer_shell": {"description": "Main outer shell", "tag": "outer_shell"},
        "top_surface": {"description": "Top planar face", "selector": 'faces(">Z")'},
    },
}


def build(params=None):
    params = params or {}
    width = params.get("width", 80)
    length = params.get("length", 60)
    height = params.get("height", 8)
    return cq.Workplane("XY").box(width, length, height).tag("outer_shell")
