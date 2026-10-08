"""
生成 ByteWriter 全套图标（方案 A：字节羽毛笔）

设计：蓝色渐变圆角方块底（左上 #60A5FA → 右下 #2563EB）+ 白色斜置羽毛笔尖，
笔尖流出 01 二进制圆点。与 Logo.vue（动态着色透明版）语义一致。

输出（Tauri v2 标准图标矩阵）：
  icons/icon.png              — 512x512 高清 PNG（通用主图标）
  icons/32x32.png             — 32x32 PNG
  icons/128x128.png           — 128x128 PNG
  icons/128x128@2x.png        — 256x256 PNG（高分屏）
  icons/icon.ico              — 多尺寸 ICO（16/24/32/48/64/128/256），Windows 任务栏/窗口

依赖：Pillow（已装）。无 SVG/cairo 依赖。

重新生成：python apps/desktop/src-tauri/icons/gen_icon.py
"""
from PIL import Image, ImageDraw
import os
import math

# 渐变端点色（与 Logo.vue / SVG 一致）
TOP_LEFT = (96, 165, 250)     # #60A5FA
BOTTOM_RIGHT = (37, 99, 235)  # #2563EB


def lerp(a, b, t):
    return a + (b - a) * t


def make_gradient(size):
    """生成对角线渐变（左上 → 右下）"""
    grad = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = grad.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * (size - 1))
            r = int(lerp(TOP_LEFT[0], BOTTOM_RIGHT[0], t))
            g = int(lerp(TOP_LEFT[1], BOTTOM_RIGHT[1], t))
            b = int(lerp(TOP_LEFT[2], BOTTOM_RIGHT[2], t))
            px[x, y] = (r, g, b, 255)
    return grad


def make_logo(size: int) -> Image.Image:
    """
    生成指定尺寸图标：蓝渐变圆角方块底 + 白色羽毛笔 + 二进制点。
    64 坐标系设计，超采样后缩放。
    """
    SS = 4  # 超采样倍数（小尺寸也保证边缘平滑）
    S = size * SS
    k = S / 64
    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    # 1. 渐变底
    grad = make_gradient(S)
    canvas.paste(grad, (0, 0))

    # 圆角遮罩：大圆角约 14k，小尺寸 12k 避免变形
    corner_r = (12 if size <= 24 else 14) * k
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S, S], radius=corner_r, fill=255)
    canvas.putalpha(mask)

    d = ImageDraw.Draw(canvas)

    # 2. 羽毛笔（白色斜杆，右上(44,14)→左下(18,46)）
    def pen_shape():
        x1, y1 = 44 * k, 14 * k
        x2, y2 = 18 * k, 46 * k
        dx, dy = x2 - x1, y2 - y1
        length = math.hypot(dx, dy)
        nx, ny = -dy / length, dx / length  # 法向
        half_w = 2.6 * k
        p1 = (x1 + nx * half_w, y1 + ny * half_w)
        p2 = (x1 - nx * half_w, y1 - ny * half_w)
        p3 = (x2 - nx * half_w, y2 - ny * half_w)
        p4 = (x2 + nx * half_w, y2 + ny * half_w)
        return [p1, p2, p3, p4], (x2, y2), (nx, ny)

    shaft, tip, (nx, ny) = pen_shape()
    d.polygon(shaft, fill=(255, 255, 255, 255))

    # 3. 笔尖铲形
    tx, ty = tip
    ux, uy = -nx, -ny  # 轴线方向（指向左下）
    spade_len = 7 * k
    spade_half = 4.2 * k
    base_l = (tx + nx * 2.6 * k, ty + ny * 2.6 * k)
    base_r = (tx - nx * 2.6 * k, ty - ny * 2.6 * k)
    end_center = (tx + ux * spade_len, ty + uy * spade_len)
    end_l = (end_center[0] + nx * spade_half, end_center[1] + ny * spade_half)
    end_r = (end_center[0] - nx * spade_half, end_center[1] - ny * spade_half)
    d.polygon([base_l, base_r, end_r, end_l], fill=(255, 255, 255, 255))

    # 笔尖中缝
    slit_start = (tx + ux * 1.5 * k, ty + uy * 1.5 * k)
    slit_end = (tx + ux * (spade_len + 0.5 * k), ty + uy * (spade_len + 0.5 * k))
    d.line([slit_start, slit_end], fill=(37, 99, 235, 255), width=max(1, int(1.2 * k)))

    # 4. 二进制圆点
    dot1 = (13 * k, 50 * k)
    dot2 = (9 * k, 53 * k)
    d.ellipse([dot1[0] - 1.9 * k, dot1[1] - 1.9 * k, dot1[0] + 1.9 * k, dot1[1] + 1.9 * k],
              fill=(255, 255, 255, int(255 * 0.92)))
    d.ellipse([dot2[0] - 1.5 * k, dot2[1] - 1.5 * k, dot2[0] + 1.5 * k, dot2[1] + 1.5 * k],
              fill=(255, 255, 255, int(255 * 0.62)))

    return canvas.resize((size, size), Image.LANCZOS)


def main():
    out_dir = os.path.dirname(os.path.abspath(__file__))

    # === PNG 矩阵（Tauri v2 标准）===
    # 通用主图标 512
    make_logo(512).save(os.path.join(out_dir, "icon.png"))
    # 标准尺寸 PNG
    for sz in (32, 128):
        make_logo(sz).save(os.path.join(out_dir, f"{sz}x{sz}.png"))
    # 高分屏 128@2x = 256
    make_logo(256).save(os.path.join(out_dir, "128x128@2x.png"))

    print("✓ PNG: icon.png(512), 32x32.png, 128x128.png, 128x128@2x.png(256)")

    # === ICO（Windows，多尺寸）===
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    make_logo(256).save(
        os.path.join(out_dir, "icon.ico"),
        format="ICO",
        sizes=ico_sizes,
    )
    print(f"✓ ICO: icon.ico ({[s[0] for s in ico_sizes]})")


if __name__ == "__main__":
    main()
