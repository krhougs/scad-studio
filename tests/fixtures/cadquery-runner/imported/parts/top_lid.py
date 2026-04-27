import cadquery as cq

from components.dimensions import HEIGHT, LENGTH, WIDTH

REFS = {"part": "top_lid", "features": {"top_surface": {"selector": 'faces(">Z")'}}}


def build(params=None):
    return cq.Workplane("XY").box(WIDTH, LENGTH, HEIGHT)
