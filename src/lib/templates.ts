const two = (n: number) => String(n).padStart(2, "0");

/** "2026-10-29" in local time. */
export function isoDate(d = new Date()) {
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

/** Fills {{title}}, {{date}} and {{time}} in a template. */
export function expandTemplate(text: string, title: string, now = new Date()) {
  const vars: Record<string, string> = {
    title,
    date: isoDate(now),
    time: `${two(now.getHours())}:${two(now.getMinutes())}`,
  };
  return text.replace(/\{\{\s*(\w+)\s*\}\}/g, (whole, name: string) => vars[name.toLowerCase()] ?? whole);
}
