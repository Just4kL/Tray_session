"""Генератор иконок навигации (только stdlib): 64x64 RGBA PNG, акцент #66C0F4."""
import struct
import zlib

ACC = (102, 192, 244, 255)
CLR = (0, 0, 0, 0)
W = H = 64


def new_canvas():
    return [[CLR for _ in range(W)] for _ in range(H)]


def blend(dst, src):
    dr, dg, db, da = dst
    sr, sg, sb, sa = src
    a = sa / 255.0
    return (
        int(sr * a + dr * (1 - a)),
        int(sg * a + dg * (1 - a)),
        int(sb * a + db * (1 - a)),
        max(sa, da),
    )


def px(cv, x, y, c):
    x, y = int(round(x)), int(round(y))
    if 0 <= x < W and 0 <= y < H:
        cv[y][x] = blend(cv[y][x], c)


def hline(cv, x0, x1, y, c):
    for x in range(min(x0, x1), max(x0, x1) + 1):
        px(cv, x, y, c)


def dot(cv, cx, cy, r, c):
    r2 = r * r
    for y in range(int(cy - r), int(cy + r) + 1):
        for x in range(int(cx - r), int(cx + r) + 1):
            if (x - cx) ** 2 + (y - cy) ** 2 <= r2:
                px(cv, x, y, c)


def ring(cv, cx, cy, r, w, c):
    for y in range(int(cy - r - w), int(cy + r + w) + 1):
        for x in range(int(cx - r - w), int(cx + r + w) + 1):
            d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if abs(d - r) <= w / 2.0:
                px(cv, x, y, c)


def line(cv, x0, y0, x1, y1, w, c):
    import math
    length = math.hypot(x1 - x0, y1 - y0)
    steps = max(int(length * 2), 1)
    for i in range(steps + 1):
        t = i / steps
        dot(cv, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, w / 2.0, c)


def tri(cv, pts, c):
    (x0, y0), (x1, y1), (x2, y2) = pts
    xs = [x0, x1, x2]
    ys = [y0, y1, y2]
    for y in range(int(min(ys)), int(max(ys)) + 1):
        row = []
        for (ax, ay), (bx, by) in [((x0, y0), (x1, y1)), ((x1, y1), (x2, y2)), ((x2, y2), (x0, y0))]:
            if (ay <= y < by) or (by <= y < ay):
                t = (y - ay) / (by - ay)
                row.append(ax + (bx - ax) * t)
        row.sort()
        for i in range(0, len(row) - 1, 2):
            hline(cv, int(row[i]), int(row[i + 1]), y, c)


def rect(cv, x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        hline(cv, x0, x1, y, c)


def save(cv, name):
    raw = b''.join(b'\x00' + b''.join(bytes(p) for p in row) for row in cv)

    def chunk(typ, data):
        c = struct.pack('>I', len(data)) + typ + data
        return c + struct.pack('>I', zlib.crc32(typ + data) & 0xFFFFFFFF)

    png = (b'\x89PNG\r\n\x1a\n'
           + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 6, 0, 0, 0))
           + chunk(b'IDAT', zlib.compress(raw, 9))
           + chunk(b'IEND', b''))
    with open(name, 'wb') as f:
        f.write(png)
    # проверка: ненулевая альфа
    opaque = sum(1 for row in cv for p in row if p[3] > 10)
    print(name, len(png), 'bytes, visible px:', opaque)


a = ACC
# 1. Сессии — треугольник play
c = new_canvas()
tri(c, [(24, 16), (24, 48), (46, 32)], a)
save(c, 'assets/nav_sessions.png')
# 2. Игры — ромб
c = new_canvas()
tri(c, [(32, 12), (50, 32), (32, 52)], a)
tri(c, [(32, 12), (32, 52), (14, 32)], a)
dot(c, 32, 32, 5, (27, 40, 56, 255))
save(c, 'assets/nav_games.png')
# 3. Будильники — колокол: купол + основание + язычок
c = new_canvas()
for y in range(16, 40):
    t = (y - 16) / 24.0
    hw = 6 + 14 * t
    hline(c, int(32 - hw), int(32 + hw), y, a)
rect(c, 14, 40, 50, 45, a)
dot(c, 32, 51, 5, a)
dot(c, 32, 12, 3, a)
save(c, 'assets/nav_alarms.png')
# 4. Таймер — часы: кольцо + стрелки
c = new_canvas()
ring(c, 32, 32, 18, 5, a)
line(c, 32, 32, 32, 18, 5, a)
line(c, 32, 32, 42, 36, 5, a)
save(c, 'assets/nav_timer.png')
# 5. Параметры — 3 слайдера
c = new_canvas()
for i, kx in ((16, 22), (30, 40), (44, 26)):
    hline(c, 12, 52, i, a)
    rect(c, kx - 4, i - 5, kx + 4, i + 5, a)
save(c, 'assets/nav_params.png')
# 6. О программе — "i": стержень + точка
c = new_canvas()
rect(c, 28, 26, 36, 52, a)
dot(c, 32, 16, 5, a)
save(c, 'assets/nav_about.png')
print('done')
