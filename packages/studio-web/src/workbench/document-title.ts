const PRODUCT_TITLE = "budn'";

export function documentTitleForFile(labelOrPath: string | null | undefined): string {
  const label = fileName(labelOrPath);
  return label ? `${label} · ${PRODUCT_TITLE}` : PRODUCT_TITLE;
}

function fileName(value: string | null | undefined): string {
  if (!value) return "";
  const segments = value.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? value;
}
