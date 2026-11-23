#!/usr/bin/env python3
"""Render the HYWOS logo (PNG + SVG)."""
import math
import os
from PIL import Image, ImageDraw, ImageFilter

S = 1024
SS = 2
W = S * SS
OUT = os.path.join(os.path.dirname(__file__), "..", "assets")


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def main():
    os.makedirs(OUT, exist_ok=True)
    img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    top = (16, 30, 66)
    bot = (4, 6, 16)
    for y in range(W):
        t = y / (W - 1)
        d.line([(0, y), (W, y)], fill=lerp(top, bot, t) + (255,))

    glow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    cx, cy = W // 2, W // 2
    for r in range(int(W * 0.42), 0, -8):
        a = int(70 * (1 - r / (W * 0.42)) ** 2)
        gd.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(46, 108, 226, a))
    glow = glow.filter(ImageFilter.GaussianBlur(28))
    img = Image.alpha_composite(img, glow)

    mask = Image.new("L", (W, W), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([0, 0, W - 1, W - 1], radius=int(W * 0.22), fill=255)
    base = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    base.paste(img, (0, 0), mask)

    rings = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    rd = ImageDraw.Draw(rings)
    for i, (rw, rh, thick, col, ang) in enumerate([
        (0.72, 0.30, 9, (110, 206, 255), -18),
        (0.62, 0.44, 6, (168, 140, 255), 24),
        (0.86, 0.18, 4, (255, 130, 196), -46),
    ]):
        ring = Image.new("RGBA", (W, W), (0, 0, 0, 0))
        rdw = ImageDraw.Draw(ring)
        box = [cx - W * rw / 2, cy - W * rh / 2, cx + W * rw / 2, cy + W * rh / 2]
        rdw.ellipse(box, outline=col + (255,), width=int(thick * SS))
        ring = ring.rotate(ang, center=(cx, cy), resample=Image.BICUBIC)
        rings = Image.alpha_composite(rings, ring)
    rings = rings.filter(ImageFilter.GaussianBlur(0.6 * SS))
    base = Image.alpha_composite(base, rings)

    stars = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    sd = ImageDraw.Draw(stars)
    for ang, rr, size in [(-18, 0.72, 7), (24, 0.44, 6), (-46, 0.86, 5), (150, 0.58, 4), (68, 0.50, 3)]:
        a = math.radians(ang)
        ex, ey = cx + math.cos(a) * W * rr / 2, cy + math.sin(a) * W * rr * 0.30 / 2
        for k in range(int(size * SS * 3), 0, -1):
            alpha = int(90 * (1 - k / (size * SS * 3)) ** 2)
            sd.ellipse([ex - k, ey - k, ex + k, ey + k], fill=(210, 238, 255, alpha))
        sd.ellipse([ex - size * SS, ey - size * SS, ex + size * SS, ey + size * SS], fill=(255, 255, 255, 255))
    base = Image.alpha_composite(base, stars)

    core = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    cd = ImageDraw.Draw(core)
    for k in range(int(W * 0.16), 0, -3):
        alpha = int(120 * (1 - k / (W * 0.16)) ** 2)
        cd.ellipse([cx - k, cy - k, cx + k, cy + k], fill=(120, 200, 255, alpha))
    core = core.filter(ImageFilter.GaussianBlur(10 * SS))
    base = Image.alpha_composite(base, core)

    mark = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    mdw = ImageDraw.Draw(mark)
    bw = int(W * 0.032)
    h = int(W * 0.20)
    left = cx - int(W * 0.075)
    right = cx + int(W * 0.075)
    mdw.rounded_rectangle([left - bw // 2, cy - h // 2, left + bw // 2, cy + h // 2], radius=bw // 2, fill=(245, 250, 255, 245))
    mdw.rounded_rectangle([right - bw // 2, cy - h // 2, right + bw // 2, cy + h // 2], radius=bw // 2, fill=(245, 250, 255, 245))
    mdw.rounded_rectangle([left, cy - bw * 0.7, right, cy + bw * 0.7], radius=bw // 2, fill=(245, 250, 255, 245))
    base = Image.alpha_composite(base, mark)

    final = base.resize((S, S), Image.LANCZOS)
    final.save(os.path.join(OUT, "logo.png"))
    final.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, "logo-256.png"))

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#101e42"/>
      <stop offset="1" stop-color="#04060f"/>
    </linearGradient>
    <radialGradient id="halo" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#5ec8ff" stop-opacity="0.55"/>
      <stop offset="0.45" stop-color="#2e6ce2" stop-opacity="0.18"/>
      <stop offset="1" stop-color="#2e6ce2" stop-opacity="0"/>
    </radialGradient>
    <filter id="soft"><feGaussianBlur stdDeviation="6"/></filter>
  </defs>
  <rect width="1024" height="1024" rx="225" fill="url(#bg)"/>
  <circle cx="512" cy="512" r="400" fill="url(#halo)"/>
  <g fill="none" stroke-linecap="round">
    <ellipse cx="512" cy="512" rx="368" ry="154" stroke="#6eceff" stroke-width="9" transform="rotate(-18 512 512)"/>
    <ellipse cx="512" cy="512" rx="317" ry="225" stroke="#a88cff" stroke-width="6" transform="rotate(24 512 512)"/>
    <ellipse cx="512" cy="512" rx="440" ry="92" stroke="#ff82c4" stroke-width="4" transform="rotate(-46 512 512)"/>
  </g>
  <circle cx="512" cy="512" r="150" fill="#78c8ff" opacity="0.35" filter="url(#soft)"/>
  <g fill="#f5faff">
    <rect x="477" y="410" width="33" height="204" rx="16"/>
    <rect x="514" y="410" width="33" height="204" rx="16"/>
    <rect x="477" y="495" width="70" height="30" rx="15"/>
  </g>
  <g fill="#ffffff">
    <circle cx="858" cy="360" r="9"/><circle cx="208" cy="702" r="7"/><circle cx="720" cy="760" r="5"/>
  </g>
</svg>
"""
    with open(os.path.join(OUT, "logo.svg"), "w") as fh:
        fh.write(svg)
    print("logo written")


if __name__ == "__main__":
    main()