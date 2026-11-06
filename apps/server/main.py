# BitMap 本地瓦片服务:生成带空间结构的位图数据,按 {z}/{x}/{y} 渲染 shading 瓦片
# 运行:python -m venv .venv && .venv/bin/pip install -r requirements.txt && .venv/bin/uvicorn main:app --port 8000
import io

import numpy as np
from fastapi import FastAPI, Response
from fastapi.middleware.cors import CORSMiddleware
from PIL import Image

SIZE = 2048
TILE = 256
MAX_Z = 11

app = FastAPI(title="wafermap-bitmap-tiles")
app.add_middleware(CORSMiddleware, allow_origins=["*"], allow_methods=["*"], allow_headers=["*"])

# —— 模拟位数据:低频场调制,FAIL 沿"山脊"聚集(空间结构接近真实缺陷分布) ——
rng = np.random.RandomState(42)
yy, xx = np.mgrid[0:SIZE, 0:SIZE]
field = np.sin(xx / 170.0) * np.sin(yy / 220.0) + 0.6 * np.sin((xx + yy) / 97.0)
field = (field - field.min()) / (field.max() - field.min())
r = rng.rand(SIZE, SIZE)
data = np.zeros((SIZE, SIZE), np.uint8)
fail_p = 0.06 + 0.6 * field**2
data[r < fail_p] = 1  # FAIL:沿场聚集
m = (data == 0) & (r >= fail_p) & (r < fail_p + 0.05)
data[m] = 2  # WARN
m = (data == 0) & (r >= 0.93)
data[m] = 3  # SKIP
m = (data == 0) & (r >= 0.89) & (r < 0.93)
data[m] = 4  # NULL

# —— 金字塔:多数 bin 层级 + fail 计数层级(yield 用) ——
levels: list[np.ndarray] = [data]
fail_counts: list[np.ndarray] = [(data == 1).astype(np.uint32) + (data == 2).astype(np.uint32)]
cur = data
curf = fail_counts[0]
while cur.shape[0] > 1:
    h, w = cur.shape
    blocks = cur.reshape(h // 2, 2, w // 2, 2)
    counts = np.stack([(blocks == b).sum(axis=(1, 3)) for b in range(5)], axis=-1)
    cur = counts.argmax(axis=-1).astype(np.uint8)
    levels.append(cur)
    fblocks = curf.reshape(h // 2, 2, w // 2, 2)
    curf = fblocks.sum(axis=(1, 3), dtype=np.uint32)
    fail_counts.append(curf)

BIN_COLORS = np.array(
    [[0x22, 0xC5, 0x5E], [0xEF, 0x44, 0x44], [0xEA, 0xB3, 0x08], [0x3B, 0x82, 0xF6], [0x1E, 0x29, 0x3B]],
    dtype=np.uint8,
)
STOPS_T = np.array([0.0, 0.15, 0.35, 0.6, 1.0])
STOPS_C = np.array(
    [[0x16, 0xA3, 0x4A], [0x84, 0xCC, 0x16], [0xEA, 0xB3, 0x08], [0xF9, 0x73, 0x16], [0xDC, 0x26, 0x26]],
    dtype=float,
)


def yield_lut(ratio: np.ndarray) -> np.ndarray:
    out = np.empty(ratio.shape + (3,), dtype=np.uint8)
    for c in range(3):
        out[..., c] = np.interp(ratio, STOPS_T, STOPS_C[:, c]).astype(np.uint8)
    return out


tile_cache: dict = {}


@app.get("/")
def info():
    return {
        "name": "wafermap-bitmap-tiles",
        "size": SIZE,
        "maxZ": MAX_Z,
        "tile": "GET /tiles/{z}/{x}/{y}.png?mode=yield|bin",
        "bins": {"0": "PASS", "1": "FAIL", "2": "WARN", "3": "SKIP", "4": "NULL"},
    }


@app.get("/bits")
def bits():
    """原始位数据(2048×2048 uint8,row-major),供前端分析面板/悬停读数用"""
    return Response(data.tobytes(), media_type="application/octet-stream")


@app.get("/tiles/{z}/{x}/{y}.png")
def tile(z: int, x: int, y: int, mode: str = "yield"):
    if not (0 <= z <= MAX_Z):
        return Response(status_code=404)
    key = (z, x, y, mode)
    if key in tile_cache:
        return Response(tile_cache[key], media_type="image/png")
    arr = levels[z]
    dim = arr.shape[0]
    x0, y0 = x * TILE, y * TILE
    if x0 >= dim or y0 >= dim:
        return Response(status_code=404)
    block = arr[y0 : y0 + TILE, x0 : x0 + TILE]
    if mode == "bin":
        rgb = BIN_COLORS[block]
    else:
        fc = fail_counts[z][y0 : y0 + TILE, x0 : x0 + TILE]
        ratio = np.minimum(1.0, fc / float(1 << (2 * z)))
        rgb = yield_lut(ratio)
    # 边缘补到 256(前端按固定瓦片尺寸拉伸)
    if rgb.shape[0] < TILE or rgb.shape[1] < TILE:
        rgb = np.pad(rgb, ((0, TILE - rgb.shape[0]), (0, TILE - rgb.shape[1]), (0, 0)), constant_values=11)
    img = Image.fromarray(rgb, "RGB")
    buf = io.BytesIO()
    img.save(buf, "PNG")
    payload = buf.getvalue()
    if len(tile_cache) > 6000:
        tile_cache.clear()
    tile_cache[key] = payload
    return Response(payload, media_type="image/png")
