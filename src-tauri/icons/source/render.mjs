import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const here = dirname(fileURLToPath(import.meta.url));
const iconsDir = join(here, "..");

const SHIELD =
  "M208,40H48A16,16,0,0,0,32,56v56c0,52.72,25.52,84.67,46.93,102.19,23.06,18.86,46,25.26,47,25.53a8,8,0,0,0,4.2,0c1-.27,23.91-6.67,47-25.53C198.48,196.67,224,164.72,224,112V56A16,16,0,0,0,208,40Z";
const CHECK =
  "M173.68,109.66l-56,56a8,8,0,0,1-11.32,0l-24-24a8,8,0,0,1,11.32-11.32L112,148.69l50.34-50.35a8,8,0,0,1,11.32,11.32Z";

const STATES = {
  ok: "#3EBA7A",
  paused: "#5B9BE8",
  warn: "#E0B33A",
  error: "#E26560",
  idle: "#8B929E",
};

function appIconSvg() {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="plate" x1="180" y1="80" x2="860" y2="960" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#5AD698"/>
      <stop offset="0.48" stop-color="#3EBA7A"/>
      <stop offset="1" stop-color="#247A54"/>
    </linearGradient>
    <linearGradient id="sheen" x1="512" y1="80" x2="512" y2="520" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#fff" stop-opacity="0.22"/>
      <stop offset="1" stop-color="#fff" stop-opacity="0"/>
    </linearGradient>
    <filter id="glyphShadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="10" stdDeviation="14" flood-color="#145C3A" flood-opacity="0.28"/>
    </filter>
  </defs>
  <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#plate)"/>
  <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#sheen)"/>
  <rect x="66.5" y="66.5" width="891" height="891" rx="206" fill="none" stroke="#fff" stroke-opacity="0.18" stroke-width="3"/>
  <g transform="translate(512 536) scale(2.52) translate(-128 -128)" filter="url(#glyphShadow)">
    <path fill="#fff" d="${SHIELD}"/>
    <path fill="#1F704A" d="${CHECK}"/>
  </g>
</svg>`;
}

function trayGlyph(kind) {
  if (kind === "paused") {
    return `<rect x="102" y="96" width="18" height="56" rx="5" fill="#fff"/>
    <rect x="136" y="96" width="18" height="56" rx="5" fill="#fff"/>`;
  }
  if (kind === "error" || kind === "warn") {
    return `<rect x="119" y="84" width="18" height="64" rx="9" fill="#fff"/>
    <circle cx="128" cy="172" r="11" fill="#fff"/>`;
  }
  if (kind === "idle") {
    return `<rect x="86" y="118" width="84" height="18" rx="9" fill="#fff"/>`;
  }
  return `<path fill="#fff" d="${CHECK}"/>`;
}

function trayIconSvg(kind, fill) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 256 256">
  <g transform="translate(128 130) scale(1.2) translate(-128 -128)">
    <path fill="${fill}" stroke="#101820" stroke-opacity="0.55" stroke-width="7" stroke-linejoin="round" d="${SHIELD}"/>
    ${trayGlyph(kind)}
  </g>
</svg>`;
}

function render(svg, width, dest) {
  const png = new Resvg(svg, {
    fitTo: { mode: "width", value: width },
    background: "rgba(0,0,0,0)",
  })
    .render()
    .asPng();
  writeFileSync(dest, png);
  console.log(`wrote ${dest}`);
}

mkdirSync(iconsDir, { recursive: true });

const appSvg = appIconSvg();
writeFileSync(join(here, "app-icon.svg"), appSvg);
render(appSvg, 1024, join(iconsDir, "app-icon.png"));

for (const [name, color] of Object.entries(STATES)) {
  const svg = trayIconSvg(name, color);
  writeFileSync(join(here, `tray-${name}.svg`), svg);
  render(svg, 128, join(iconsDir, `tray-${name}.png`));
  render(svg, 16, join(here, `preview-${name}-16.png`));
}
