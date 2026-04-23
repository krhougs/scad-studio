// Phase 7 smoke fixture for parameter/preset editing. Web clients cannot
// read .scad source through FileRead (server deny list), so parameter
// editing in web is driven by user-entered key=value overrides (defines),
// not by parsing this source. The file exists so PreviewRequest works.
size = 10;
wall = 2;
cube([size, size, size]);
