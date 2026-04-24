export type FileKindEntry = {
  label: string;
  kind: "file" | "directory";
};

const KNOWN_EXTENSIONS = new Map<string, string>([
  ["scad", "SCAD"],
  ["stl", "STL"],
  ["3mf", "3MF"],
  ["md", "MD"],
  ["markdown", "MD"],
  ["png", "PNG"],
  ["jpg", "JPG"],
  ["jpeg", "JPEG"],
  ["gif", "GIF"],
  ["webp", "WEBP"],
  ["bmp", "BMP"],
  ["tif", "TIF"],
  ["tiff", "TIFF"],
  ["ico", "ICO"],
  ["svg", "SVG"],
  ["json", "JSON"],
  ["txt", "TXT"],
]);

export function fileKindLabel(entry: FileKindEntry): string {
  if (entry.kind === "directory") return "DIR";
  const extension = extensionOf(entry.label);
  if (!extension) return "FILE";
  return KNOWN_EXTENSIONS.get(extension) ?? extension.toUpperCase();
}

function extensionOf(label: string): string {
  const lastSegment = label.split(/[\\/]/).pop() ?? label;
  const dot = lastSegment.lastIndexOf(".");
  if (dot <= 0 || dot === lastSegment.length - 1) return "";
  return lastSegment.slice(dot + 1).toLowerCase();
}
