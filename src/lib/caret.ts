const COPIED = [
  "boxSizing", "width", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
  "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth",
  "fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "tabSize",
  "whiteSpace", "wordWrap", "wordBreak",
] as const;

/**
 * Pixel position of character `pos` inside a textarea, relative to the
 * textarea's top-left corner (scroll accounted for). Works by laying out the
 * same text in an invisible mirror div and measuring a marker span.
 */
export function caretCoords(ta: HTMLTextAreaElement, pos: number) {
  const style = getComputedStyle(ta);
  const mirror = document.createElement("div");
  for (const prop of COPIED) mirror.style[prop] = style[prop];
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.overflowWrap = "break-word";
  mirror.textContent = ta.value.slice(0, pos);
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.appendChild(marker);
  document.body.appendChild(mirror);
  const coords = {
    x: marker.offsetLeft - ta.scrollLeft,
    y: marker.offsetTop - ta.scrollTop,
    lineHeight: marker.offsetHeight,
  };
  mirror.remove();
  return coords;
}
