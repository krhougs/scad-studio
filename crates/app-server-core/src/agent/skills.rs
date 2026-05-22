use app_server_protocol::AgentMode;

pub struct SkillInjectionContext {
    pub mode: AgentMode,
    pub cadquery_execution_tools_registered: bool,
    pub last_turn_cadquery_error: bool,
}

struct AgentSkill {
    name: &'static str,
    condition: fn(&SkillInjectionContext) -> bool,
    text: fn(&SkillInjectionContext) -> &'static str,
}

const SKILLS: &[AgentSkill] = &[
    AgentSkill {
        name: "failure-recovery",
        condition: cadquery_agent_mode,
        text: failure_recovery_text,
    },
    AgentSkill {
        name: "engineering-defaults",
        condition: cadquery_agent_mode,
        text: |_| ENGINEERING_DEFAULTS_SKILL,
    },
    AgentSkill {
        name: "structured-brief",
        condition: cadquery_agent_mode,
        text: |_| STRUCTURED_BRIEF_SKILL,
    },
];

fn cadquery_agent_mode(ctx: &SkillInjectionContext) -> bool {
    ctx.mode == AgentMode::Agent && ctx.cadquery_execution_tools_registered
}

fn failure_recovery_text(ctx: &SkillInjectionContext) -> &'static str {
    if ctx.last_turn_cadquery_error {
        FAILURE_RECOVERY_FULL_SKILL
    } else {
        FAILURE_RECOVERY_COMPACT_SKILL
    }
}

pub fn collect_skill_injections(ctx: &SkillInjectionContext) -> String {
    let mut sections = Vec::new();
    for skill in SKILLS {
        if (skill.condition)(ctx) {
            sections.push(format!("[Skill: {}]\n{}", skill.name, (skill.text)(ctx)));
        }
    }
    if sections.is_empty() {
        return String::new();
    }
    format!("Active skills:\n{}", sections.join("\n\n"))
}

pub fn active_skill_names(ctx: &SkillInjectionContext) -> Vec<&'static str> {
    SKILLS
        .iter()
        .filter(|s| (s.condition)(ctx))
        .map(|s| s.name)
        .collect()
}

const FAILURE_RECOVERY_COMPACT_SKILL: &str = "\
If cadquery_dry_run or cadquery_execute fails during this turn, apply the following before retrying:
- Syntax error: fix the Python error and dry_run again.
- Geometry invalid (empty shape / self-intersection / open shell): simplify the operation or split into smaller steps.
- Fillet/chamfer overflow: reduce radius to ≤ half the shortest adjacent edge length, then dry_run.
- Scale mismatch (dimension off by >2×): re-check units and parameter values.
- Boolean failure (empty intersection / tangent degeneration): add 0.01 mm offset or reorder operations.
- Assembly positioning failure: verify mate references exist and are not coplanar.
Maximum 2 automatic retries per turn. Each retry must dry_run before execute.";

const FAILURE_RECOVERY_FULL_SKILL: &str = "\
The previous turn had a CadQuery execution error. Use the following classification to guide your fix:

Failure classification and repair procedures:
1. **Syntax error** — Fix the Python traceback. Common: missing import, wrong API name, indent error.
2. **Geometry invalid** (empty shape / self-intersection / open shell) — The operation produced degenerate geometry. Simplify: use fewer boolean steps, avoid near-zero thickness, split complex sweeps.
3. **Fillet/chamfer overflow** — Radius too large for the edge neighborhood. Reduce radius to ≤ half the shortest adjacent edge. If uncertain, start at 0.5 mm and increase.
4. **Scale mismatch** (output bounding box differs from intent by >2×) — Re-check unit assumptions (mm vs m), parameter naming, and reference dimensions.
5. **Boolean failure** (intersection is empty / tangent degeneration) — Bodies may be just touching or not overlapping. Add 0.01 mm offset to ensure proper overlap, or reorder cut/fuse sequence.
6. **Assembly positioning failure** — A mate reference is missing, renamed, or coplanar. Verify that both mate faces exist in the current topology and are not parallel.

Rules:
- Maximum 2 automatic retries per turn. Each retry must dry_run before execute.
- If 2 retries fail, report the failure class, what you tried, and suggest a manual fix to the user.
- Do not retry with identical code — each attempt must change something based on the diagnosis above.";

const ENGINEERING_DEFAULTS_SKILL: &str = "\
When generating or modifying a CadQuery model and a parameter is not specified by the user, \
you may use the following 3D printing defaults as assumptions. \
Always declare which defaults you used in your response.

3D printing defaults:
- Wall thickness: 2–3 mm
- Fillet/chamfer radius: 0.5–1.5 mm
- Minimum feature size: 0.8 mm
- Clearance hole diameters: M2=2.4 mm, M3=3.4 mm, M4=4.5 mm, M5=5.5 mm, M6=6.6 mm
- Coordinate convention: Z-up, origin at bottom geometric center
- Print tolerance: ±0.2 mm; CNC tolerance: ±0.05 mm

These are assumptions, not facts. If the user specifies different values, use those instead. \
For hardware-specific dimensions (board sizes, connector footprints), use web_search — do not guess.";

const STRUCTURED_BRIEF_SKILL: &str = "\
Before generating CadQuery code for a new model or a large modification, output a brief that \
summarizes your plan. The brief helps the user verify your assumptions before you commit code.

Output a brief when:
- Creating a new model (no existing CadQuery result in the workspace, or user explicitly requests new)
- The request leaves multiple parameters unspecified (dimensions, material, process)
- The modification affects multiple features or components

Do NOT output a brief for:
- Simple single-parameter changes (\"change height to 50 mm\", \"fillet 1 mm\")
- Query or describe operations (\"describe this face\")
- Plan mode execution steps (the plan already contains intent)

Brief format (output as plain text in your message, not as a tool call):
**Brief**
- Target dimensions: [overall size]
- Key features: [list of planned features]
- Assumptions: [list defaults used, e.g. \"wall thickness 2 mm (3D printing default)\"]
- Verification: [what to check after generation]
- REFS.features plan: [planned feature keys]

After outputting the brief, proceed with code generation in the same turn — do not wait for confirmation.";
