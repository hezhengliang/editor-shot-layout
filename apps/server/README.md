# BitMap 瓦片服务(本地 FastAPI)

为 BitMapView 的 Tile Source 模式提供后端 shading 瓦片。

```bash
cd server
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/uvicorn main:app --port 8000
```

前端 BitMapView 侧栏 Tile Source 填入:

```
http://localhost:8000/tiles/{z}/{x}/{y}.png?mode=yield
```

- `mode=yield`(默认):按块内 fail 比例渲染 绿→红 良率渐变
- `mode=bin`:按多数 bin 着色
- z = 聚合层级(2^z bit/像素,0=逐 bit,11=整图 1 像素);x/y = 瓦片索引;单张 256×256 PNG
- 数据为固定随机种子生成的模拟位图(FAIL 沿低频场聚集,带空间结构);换成真实数据时替换 `data` 的生成即可,金字塔与渲染逻辑不变
