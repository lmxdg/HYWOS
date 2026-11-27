#!/usr/bin/env python3
"""Render the HYWOS logo: a minimal geometric mark (PNG + SVG)."""
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

    top = (13, 22, 40)
    bot = (6, 10, 20)
    for y in range(W):
        t = y / (W - 1)
        d.line([(0, y), (W, y)], fill=lerp(top, bot, t) + (255,))

    mask = Image.new("L", (W, W), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([0, 0, W - 1, W - 1], radius=int(W * 0.23), fill=255)
    base = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    base.paste(img, (0, 0), mask)

    cx = cy = W // 2

    ring = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    rd = ImageDraw.Draw(ring)
    rr = int(W * 0.345)
    rw = max(2, int(W * 0.0045))
    rd.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], outline=(58, 96, 148, 150), width=rw)
    base = Image.alpha_composite(base, ring)

    mark = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    mk = ImageDraw.Draw(mark)
    bar_w = int(W * 0.062)
    bar_h = int(W * 0.360)
    gap = int(W * 0.098)
    rad = bar_w // 2
    left = cx - gap - bar_w // 2
    right = cx + gap - bar_w // 2
    mark_top = cy - bar_h // 2
    mark_bot = cy + bar_h // 2
    white = (242, 247, 255, 252)
    mk.rounded_rectangle([left, mark_top, left + bar_w, mark_bot], radius=rad, fill=white)
    mk.rounded_rectangle([right, mark_top, right + bar_w, mark_bot], radius=rad, fill=white)
    cross_h = int(W * 0.062)
    mk.rounded_rectangle([left, cy - cross_h // 2, right + bar_w, cy + cross_h // 2], radius=cross_h // 2, fill=white)

    accent = int(W * 0.017)
    ax = right + bar_w + int(W * 0.052)
    ay = cy - int(W * 0.115)
    for k in range(int(W * 0.05), 0, -2):
        a = int(60 * (1 - k / (W * 0.05)) ** 2)
        mk.ellipse([ax - k, ay - k, ax + k, ay + k], fill=(76, 194, 255, a))
    mk.ellipse([ax - accent, ay - accent, ax + accent, ay + accent], fill=(76, 194, 255, 250))
    base = Image.alpha_composite(base, mark)

    final = base.resize((S, S), Image.LANCZOS)
    final.save(os.path.join(OUT, "logo.png"))
    final.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, "logo-256.png"))

    scale = S / 1024.0
    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#0d1628"/>
      <stop offset="1" stop-color="#060a14"/>
    </linearGradient>
  </defs>
  <rect width="1024" height="1024" rx="235" fill="url(#bg)"/>
  <circle cx="512" cy="512" r="353" fill="none" stroke="#3a6094" stroke-opacity="0.59" stroke-width="4.6"/>
  <g fill="#f2f7ff">
    <rect x="378" y="328" width="64" height="368" rx="32"/>
    <rect x="582" y="328" width="64" height="368" rx="32"/>
    <rect x="378" y="480" width="268" height="64" rx="32"/>
  </g>
  <circle cx="725" cy="394" r="17" fill="#4cc2ff"/>
</svg>
"""
    with open(os.path.join(OUT, "logo.svg"), "w") as fh:
        fh.write(svg)
    print("logo written")


if __name__ == "__main__":
    main()