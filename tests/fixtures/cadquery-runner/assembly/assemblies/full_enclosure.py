import cadquery as cq

from components.pcb_main import build as build_pcb
from parts.bottom_case import build as build_bottom_case
from parts.top_lid import build as build_top_lid

REFS = {
    "assembly": "full_enclosure",
    "children": ["bottom_case", "top_lid", "pcb_main"],
}


def build(params=None):
    assembly = cq.Assembly(name="full_enclosure")
    assembly.add(
        build_bottom_case(),
        name="bottom_case",
        metadata={"ref_text": "@part[bottom_case]", "object_kind": "part"},
    )
    assembly.add(
        build_top_lid(),
        name="top_lid",
        loc=cq.Location(cq.Vector(0, 0, 9)),
        metadata={"ref_text": "@part[top_lid]", "object_kind": "part"},
    )
    assembly.add(
        build_pcb(),
        name="pcb_main",
        loc=cq.Location(cq.Vector(0, 0, 2)),
        metadata={"ref_text": "@component[pcb_main]", "object_kind": "component"},
    )
    return assembly
