"""Flotilla icon set from one geometric definition (logo #J).

Coordinates are in a 512-unit motif space measured from the chosen
generated image; the motif is placed on an Apple-style continuous-corner
rounded square (macOS), a full-bleed square (iOS) or a tighter rounded
square (Windows, web), and rendered with 4x supersampling.
"""
import math
import os
import sys

from PIL import Image, ImageDraw

SAND = (236, 221, 188)
CORAL = (241, 77, 65)
NAVY = (12, 48, 98)
CX = 256  # motif's axis of symmetry


def mirror(points):
    return [(2 * CX - x, y) for x, y in points]


def sail(center):
    """A navy sail: two halves with a gap on the centre line."""
    d = center - 152
    left = [(149, 255), (102, 333), (148, 340)]
    right = [(155, 255), (202, 333), (156, 340)]
    return [[(x + d, y) for x, y in half] for half in (left, right)]


def hull(center, top, width, depth, dip):
    h = width / 2
    return [(center - h, top), (center, top + dip), (center + h, top), (center, top + depth)]


def coral_shapes():
    shapes = []
    # Mast: a thin bar with a rounded top, drawn as a polygon plus a cap.
    shapes.append(("rect", (211, 99, 217, 262)))
    shapes.append(("cap", (214, 99, 3)))
    # Three kanban columns; their tops follow the sail's diagonal.
    slope = 1.0
    for x0, x1 in ((224, 250), (257, 282), (290, 315)):
        y0 = 109 + (x0 - 224) * slope
        y1 = 109 + (x1 - 224) * slope
        shapes.append(("poly", [(x0, y0), (x1, y1), (x1, 262), (x0, 262)]))
    shapes.append(("poly", hull(CX, 271, 104, 44, 6)))
    return shapes


def navy_shapes():
    out = []
    for c in (152, 360):
        for half in sail(c):
            out.append(("poly", half))
        out.append(("poly", hull(c, 344, 60, 27, 5)))
    return out


MOTIF_BOX = (102, 96, 410, 371)  # bounds of everything above


def superellipse(cx, cy, half, n=5.0, steps=720):
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = half * math.copysign(abs(c) ** (2 / n), c)
        y = half * math.copysign(abs(s) ** (2 / n), s)
        pts.append((cx + x, cy + y))
    return pts


def render(size, plate, glyph_frac, ss=4):
    """plate: 'macos' (squircle with Apple margins), 'full' (opaque square),
    'tight' (squircle filling the canvas). glyph_frac: motif width / plate."""
    S = size * ss
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if plate == "macos":
        side = S * 824 / 1024
    elif plate == "tight":
        side = S * 0.94
    else:
        side = S
    if plate == "full":
        d.rectangle((0, 0, S, S), fill=SAND)
    else:
        d.polygon(superellipse(S / 2, S / 2, side / 2), fill=SAND)
    mx0, my0, mx1, my1 = MOTIF_BOX
    mw, mh = mx1 - mx0, my1 - my0
    scale = side * glyph_frac / mw
    ox = S / 2 - (mx0 + mw / 2) * scale
    oy = S / 2 - (my0 + mh / 2) * scale + side * 0.01  # optical centre

    def tx(points):
        return [(ox + x * scale, oy + y * scale) for x, y in points]

    for colour, shapes in ((NAVY, navy_shapes()), (CORAL, coral_shapes())):
        for kind, data in shapes:
            if kind == "poly":
                d.polygon(tx(data), fill=colour)
            elif kind == "rect":
                x0, y0, x1, y1 = data
                d.rectangle(tx([(x0, y0), (x1, y1)]), fill=colour)
            elif kind == "cap":
                x, y, r = data
                (px, py), = tx([(x, y)])
                d.ellipse((px - r * scale, py - r * scale, px + r * scale, py + r * scale), fill=colour)
    return img.resize((size, size), Image.LANCZOS)


def svg(plate="macos", glyph_frac=0.62):
    """The same mark as SVG (1024 canvas)."""
    S = 1024
    side = S * 824 / 1024 if plate == "macos" else S * 0.94 if plate == "tight" else S
    mx0, my0, mx1, my1 = MOTIF_BOX
    mw, mh = mx1 - mx0, my1 - my0
    scale = side * glyph_frac / mw
    ox = S / 2 - (mx0 + mw / 2) * scale
    oy = S / 2 - (my0 + mh / 2) * scale + side * 0.01

    def pts(points):
        return " ".join(f"{ox + x * scale:.2f},{oy + y * scale:.2f}" for x, y in points)

    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" width="{S}" height="{S}">',
             "<title>Flotilla</title>"]
    if plate == "full":
        parts.append(f'<rect width="{S}" height="{S}" fill="#ECDDBC"/>')
    else:
        parts.append(f'<polygon fill="#ECDDBC" points="{pts_plain(superellipse(S / 2, S / 2, side / 2, steps=240))}"/>')
    for colour, shapes in (("#0C3062", navy_shapes()), ("#F14D41", coral_shapes())):
        for kind, data in shapes:
            if kind == "poly":
                parts.append(f'<polygon fill="{colour}" points="{pts(data)}"/>')
            elif kind == "rect":
                x0, y0, x1, y1 = data
                parts.append(f'<rect fill="{colour}" x="{ox + x0 * scale:.2f}" y="{oy + y0 * scale:.2f}" '
                             f'width="{(x1 - x0) * scale:.2f}" height="{(y1 - y0) * scale:.2f}"/>')
            elif kind == "cap":
                x, y, r = data
                parts.append(f'<circle fill="{colour}" cx="{ox + x * scale:.2f}" cy="{oy + y * scale:.2f}" r="{r * scale:.2f}"/>')
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


def pts_plain(points):
    return " ".join(f"{x:.2f},{y:.2f}" for x, y in points)


if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    render(1024, "macos", 0.66).save(f"{out}/icon-macos-1024.png")
    render(1024, "full", 0.56).convert("RGB").save(f"{out}/icon-ios-1024.png")
    render(1024, "tight", 0.66).save(f"{out}/icon-tight-1024.png")
    for s in (16, 32, 48, 180, 192, 512):
        render(s, "tight", 0.70 if s <= 48 else 0.66).save(f"{out}/favicon-{s}.png")
    open(f"{out}/icon-macos.svg", "w").write(svg("macos", 0.66))
    open(f"{out}/icon-tight.svg", "w").write(svg("tight", 0.66))
    open(f"{out}/icon-ios.svg", "w").write(svg("full", 0.56))
    print("written to", out)
