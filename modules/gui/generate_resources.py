import os
import shutil
from PIL import Image, ImageDraw, ImageFilter


_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
_ICONS_DIR = os.path.join(_SCRIPT_DIR, "resources", "icons")
_IMAGES_DIR = os.path.join(_SCRIPT_DIR, "resources", "images")

def create_dirs():
    os.makedirs(_ICONS_DIR, exist_ok=True)
    os.makedirs(_IMAGES_DIR, exist_ok=True)

def generate_gradients():
    covers = {
        "cover_midnight_dreams.png": (("#1D0B3A", "#C9246B"), ("#C9246B", "#E95D35")),
        "cover_echoes.png": (("#0A1128", "#001F54"), ("#001F54", "#1282A2")),
        "cover_starlight.png": (("#3D0C02", "#800E13"), ("#800E13", "#F7B32B")),
        "cover_endless_road.png": (("#1E0034", "#3D0066"), ("#3D0066", "#00F5D4")),
        "cover_breathe_again.png": (("#1B4965", "#62B6CB"), ("#62B6CB", "#FFE5EC")),
        "cover_sailing_home.png": (("#012A4A", "#01497C"), ("#01497C", "#89C2D9")),
        "cover_letting_go.png": (("#5F0F40", "#9A031E"), ("#9A031E", "#E36414")),
        "cover_golden_hours.png": (("#4F5D75", "#EF8354"), ("#EF8354", "#F4C430")),
        "cover_night_drive.png": (("#0D1F2D", "#1D2D44"), ("#1D2D44", "#748CAB"))
    }


    try:
        import numpy as np
        _use_numpy = True
    except ImportError:
        _use_numpy = False

    for filename, (color1, color2) in covers.items():
        # Parse hex colors once.
        def hex2rgb(h):
            return (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16))
        r1, g1, b1 = hex2rgb(color1[0])
        r2, g2, b2 = hex2rgb(color1[1])
        r3, g3, b3 = hex2rgb(color2[0])
        r4, g4, b4 = hex2rgb(color2[1])

        if _use_numpy:
            # Vectorized gradient via numpy broadcasting.
            ys = np.linspace(0, 1, 300).reshape(-1, 1)
            xs = np.linspace(0, 1, 300).reshape(1, -1)
            top = np.array([r1, g1, b1]) + (np.array([r2, g2, b2]) - np.array([r1, g1, b1])) * ys
            bot = np.array([r3, g3, b3]) + (np.array([r4, g4, b4]) - np.array([r3, g3, b3])) * ys
            # top: (300, 3), xs: (1, 300) → broadcast to (300, 300, 3)
            top_b = top[:, None, :]  # (300, 1, 3)
            bot_b = bot[:, None, :]  # (300, 1, 3)
            rgb = top_b + (bot_b - top_b) * xs[:, :, None]
            rgba = np.dstack([rgb.astype(np.uint8), np.full((300, 300), 255, dtype=np.uint8)])
            img = Image.fromarray(rgba, "RGBA")
        else:
            # Fallback: build a flat bytearray and load it in one shot.
            buf = bytearray(300 * 300 * 4)
            for y in range(300):
                frac_y = y / 299.0 if 300 > 1 else 0.0
                r_top = int(r1 + (r2 - r1) * frac_y)
                g_top = int(g1 + (g2 - g1) * frac_y)
                b_top = int(b1 + (b2 - b1) * frac_y)
                r_bot = int(r3 + (r4 - r3) * frac_y)
                g_bot = int(g3 + (g4 - g3) * frac_y)
                b_bot = int(b3 + (b4 - b3) * frac_y)
                row_off = y * 300 * 4
                for x in range(300):
                    frac_x = x / 299.0 if 300 > 1 else 0.0
                    buf[row_off + x*4 + 0] = int(r_top + (r_bot - r_top) * frac_x)
                    buf[row_off + x*4 + 1] = int(g_top + (g_bot - g_top) * frac_x)
                    buf[row_off + x*4 + 2] = int(b_top + (b_bot - b_top) * frac_x)
                    buf[row_off + x*4 + 3] = 255
            img = Image.frombytes("RGBA", (300, 300), bytes(buf))

        # Add a glowing circle in the middle to simulate a sun/moon.
        draw = ImageDraw.Draw(img)
        draw.ellipse([80, 80, 220, 220], fill=(255, 255, 255, 30))
        draw.ellipse([110, 110, 190, 190], fill=(255, 255, 255, 60))

        img.save(os.path.join(_IMAGES_DIR, filename))

def draw_icon(name, draw_fn):
    # Draw at 64x64 for high DPI scaling
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw_fn(draw)
    # P-137: use absolute path resolved at module load.
    img.save(os.path.join(_ICONS_DIR, f"{name}.png"))

def draw_home(d):
    d.polygon([(32, 8), (8, 28), (56, 28)], fill=None, outline="white", width=4)
    d.rectangle([14, 28, 50, 56], fill=None, outline="white", width=4)
    d.rectangle([24, 40, 40, 56], fill="white")

def draw_albums(d):
    d.ellipse([10, 10, 54, 54], fill=None, outline="white", width=4)
    d.ellipse([22, 22, 42, 42], fill=None, outline="white", width=3)
    d.ellipse([29, 29, 35, 35], fill="white")

def draw_artists(d):
    d.ellipse([22, 10, 42, 30], fill=None, outline="white", width=4)
    # Shoulder arc
    d.arc([10, 36, 54, 76], 180, 360, fill="white", width=4)

def draw_folders(d):
    d.line([(8, 16), (24, 16), (32, 24), (56, 24), (56, 52), (8, 52), (8, 16)], fill="white", width=4, joint="round")
    d.line([(8, 24), (28, 24)], fill="white", width=4)

def draw_favorites(d):
    # Draw a filled heart for Favorites
    # Simple heart shape using polygons and circles
    # We can draw it using a path-like approach
    # Let's draw it using circles + polygon
    d.ellipse([14, 16, 32, 34], fill="white")
    d.ellipse([32, 16, 50, 34], fill="white")
    d.polygon([(14, 26), (50, 26), (32, 52)], fill="white")

def draw_recently_played(d):
    d.ellipse([10, 10, 54, 54], fill=None, outline="white", width=4)
    d.line([(32, 18), (32, 32), (44, 32)], fill="white", width=4)

def draw_most_played(d):
    # Bar Chart
    d.rectangle([10, 36, 20, 54], fill="white")
    d.rectangle([27, 18, 37, 54], fill="white")
    d.rectangle([44, 28, 54, 54], fill="white")

def draw_settings(d):
    d.ellipse([24, 24, 40, 40], fill=None, outline="white", width=4)
    # Simple spokes
    for i in range(8):
        import math
        angle = i * math.pi / 4
        x1 = 32 + 20 * math.cos(angle)
        y1 = 32 + 20 * math.sin(angle)
        x2 = 32 + 27 * math.cos(angle)
        y2 = 32 + 27 * math.sin(angle)
        d.line([(x1, y1), (x2, y2)], fill="white", width=5)

def draw_plus(d):
    d.line([(32, 12), (32, 52)], fill="white", width=5)
    d.line([(12, 32), (52, 32)], fill="white", width=5)

def draw_play(d):
    d.polygon([(18, 12), (52, 32), (18, 52)], fill="white")

def draw_pause(d):
    d.rectangle([16, 12, 26, 52], fill="white")
    d.rectangle([38, 12, 48, 52], fill="white")

def draw_prev(d):
    d.polygon([(46, 12), (20, 32), (46, 52)], fill="white")
    d.rectangle([12, 12, 18, 52], fill="white")

def draw_next(d):
    d.polygon([(18, 12), (44, 32), (18, 52)], fill="white")
    d.rectangle([46, 12, 52, 52], fill="white")

def draw_repeat(d):
    # Circular arrow
    d.arc([12, 12, 52, 52], 45, 315, fill="white", width=4)
    d.polygon([(46, 20), (56, 32), (38, 34)], fill="white")

def draw_shuffle(d):
    # Two crossed arrows
    d.line([(10, 16), (28, 16), (46, 48), (54, 48)], fill="white", width=4)
    d.line([(10, 48), (28, 48), (46, 16), (54, 16)], fill="white", width=4)
    # Arrow heads
    d.polygon([(54, 16), (44, 10), (46, 22)], fill="white")
    d.polygon([(54, 48), (44, 42), (46, 54)], fill="white")

def draw_equalizer(d):
    # 3 sliders
    # Slider tracks
    d.line([(16, 10), (16, 54)], fill="white", width=2)
    d.line([(32, 10), (32, 54)], fill="white", width=2)
    d.line([(48, 10), (48, 54)], fill="white", width=2)
    # Knobs
    d.ellipse([10, 36, 22, 48], fill="white")
    d.ellipse([26, 16, 38, 28], fill="white")
    d.ellipse([42, 28, 54, 40], fill="white")

def draw_volume(d):
    # Speaker outline
    d.polygon([(26, 18), (14, 26), (6, 26), (6, 38), (14, 38), (26, 46)], fill="white")
    # Wave arcs
    d.arc([16, 16, 40, 48], -45, 45, fill="white", width=3)
    d.arc([12, 8, 48, 56], -45, 45, fill="white", width=3)

def draw_search(d):
    d.ellipse([14, 14, 40, 40], fill=None, outline="white", width=4)
    d.line([(34, 34), (52, 52)], fill="white", width=5)

def draw_more(d):
    # Three dots
    d.ellipse([29, 13, 35, 19], fill="white")
    d.ellipse([29, 29, 35, 35], fill="white")
    d.ellipse([29, 45, 35, 51], fill="white")

def draw_list(d):
    # List lines
    d.line([(22, 18), (54, 18)], fill="white", width=4)
    d.line([(22, 32), (54, 32)], fill="white", width=4)
    d.line([(22, 46), (54, 46)], fill="white", width=4)
    # Bullets
    d.ellipse([10, 15, 16, 21], fill="white")
    d.ellipse([10, 29, 16, 35], fill="white")
    d.ellipse([10, 43, 16, 49], fill="white")

def draw_grid(d):
    d.rectangle([10, 10, 26, 26], fill=None, outline="white", width=4)
    d.rectangle([38, 10, 54, 26], fill=None, outline="white", width=4)
    d.rectangle([10, 38, 26, 54], fill=None, outline="white", width=4)
    d.rectangle([38, 38, 54, 54], fill=None, outline="white", width=4)

def draw_close(d):
    d.line([(16, 16), (48, 48)], fill="white", width=5)
    d.line([(48, 16), (16, 48)], fill="white", width=5)

def draw_info(d):
    d.ellipse([10, 10, 54, 54], fill=None, outline="white", width=4)
    d.line([(32, 26), (32, 44)], fill="white", width=4)
    d.ellipse([30, 18, 34, 22], fill="white")

def draw_reset(d):
    d.arc([10, 10, 54, 54], 45, 360, fill="white", width=4)
    d.polygon([(36, 6), (54, 14), (40, 28)], fill="white")

def main():
    create_dirs()
    generate_gradients()
    
    icons = {
        "home": draw_home,
        "albums": draw_albums,
        "artists": draw_artists,
        "folders": draw_folders,
        "favorites": draw_favorites,
        "recently_played": draw_recently_played,
        "most_played": draw_most_played,
        "settings": draw_settings,
        "plus": draw_plus,
        "play": draw_play,
        "pause": draw_pause,
        "prev": draw_prev,
        "next": draw_next,
        "repeat": draw_repeat,
        "shuffle": draw_shuffle,
        "equalizer": draw_equalizer,
        "volume": draw_volume,
        "search": draw_search,
        "more": draw_more,
        "list": draw_list,
        "grid": draw_grid,
        "info": draw_info,
        "reset": draw_reset,
        "close": draw_close
    }
    
    for name, fn in icons.items():
        draw_icon(name, fn)
    
    # Copy app logo from repo assets folder if present
    logo_src = os.path.abspath(os.path.join(_SCRIPT_DIR, "..", "..", "assets", "playtune_logo.png"))
    if os.path.exists(logo_src):
        shutil.copy(logo_src, os.path.join(_ICONS_DIR, "playtune_logo.png"))
        
    print("Successfully generated all assets.")

if __name__ == "__main__":
    main()
