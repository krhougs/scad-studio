import cadquery as cq

from components.pcb_main import REFS as PCB_MAIN_REFS, build as build_pcb
from parts.bottom_case import REFS as BOTTOM_CASE_REFS, build as build_bottom_case
from parts.top_lid import REFS as TOP_LID_REFS, build as build_top_lid

REFS = {
    "assembly": "full_enclosure",
    "children": ["bottom_case", "top_lid", "pcb_main"],
}


def build(params=None):
    assembly = cq.Assembly(name="full_enclosure")
    assembly.add(
        build_bottom_case(),
        name="bottom_case",
        metadata={
            "ref_text": "@part[bottom_case]",
            "object_kind": "part",
            "features": BOTTOM_CASE_REFS["features"],
        },
    )
    assembly.add(
        build_top_lid(),
        name="top_lid",
        loc=cq.Location(cq.Vector(0, 0, 9)),
        metadata={
            "ref_text": "@part[top_lid]",
            "object_kind": "part",
            "features": TOP_LID_REFS["features"],
        },
    )
    assembly.add(
        build_pcb(),
        name="pcb_main",
        loc=cq.Location(cq.Vector(0, 0, 2)),
        metadata={
            "ref_text": "@component[pcb_main]",
            "object_kind": "component",
            "features": PCB_MAIN_REFS["features"],
        },
    )
    return assembly
