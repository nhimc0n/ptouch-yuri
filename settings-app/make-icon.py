# SPDX-License-Identifier: GPL-3.0-or-later
"""Draws the app icon (icon-source.png, 1024x1024). Needs Pillow.

Run `python3 make-icon.py && (cd src-tauri && cargo tauri icon ../icon-source.png)`
to regenerate every size in src-tauri/icons. Colours are the design system's
(see DESIGN.md): navy background, white label, blue accent.
"""
from PIL import Image, ImageDraw

SIZE, SCALE = 1024, 4          # draw at 4096 and shrink, for smooth edges
S = SIZE * SCALE
NAVY, NAVY_LIGHT = (30, 58, 95), (43, 77, 122)
WHITE, INK, BLUE, GREEN = (255, 255, 255), (30, 58, 95), (37, 99, 235), (5, 150, 105)

def px(v):
    return round(v * SCALE)

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# macOS icon grid: the artwork sits inside an 824 px square on a 1024 canvas.
margin, radius = px(100), px(185)
box = (margin, margin, S - margin, S - margin)

# vertical gradient inside the rounded square
gradient = Image.new("RGBA", (S, S))
gd = ImageDraw.Draw(gradient)
for y in range(margin, S - margin):
    t = (y - margin) / (S - 2 * margin)
    gd.line([(0, y), (S, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(NAVY_LIGHT, NAVY)) + (255,))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle(box, radius=radius, fill=255)
img.paste(gradient, (0, 0), mask)

d = ImageDraw.Draw(img)
# the label: a white strip, wider than tall, with the printed end cut straight
lx0, ly0, lx1, ly1 = px(210), px(372), px(814), px(652)
d.rounded_rectangle((lx0, ly0, lx1, ly1), radius=px(34), fill=WHITE)
# printed "text": one long bar, one short bar
d.rounded_rectangle((px(262), px(430), px(640), px(486)), radius=px(28), fill=INK)
d.rounded_rectangle((px(262), px(538), px(500), px(582)), radius=px(22), fill=BLUE)
# the cut: dashed line near the right end and a green mark (ready)
for y in range(372 + 24, 652 - 20, 44):
    d.rounded_rectangle((px(742), px(y), px(756), px(y + 24)), radius=px(7), fill=(180, 190, 205))
d.ellipse((px(676), px(520), px(716), px(560)), fill=GREEN)

out = img.resize((SIZE, SIZE), Image.LANCZOS)
out.save("icon-source.png")
print("icon-source.png written")
