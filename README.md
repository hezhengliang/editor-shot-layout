# editor-shot-layout

基于 leafer-editor 的半导体 Shot/Die 布局编辑器:无限画布 + 标尺,Shot 槽位阵列(内圈/外圈 scribe 标注),Die 模板库与填充,晶圆图与 Bit Map 检视;Tweakpane 属性面板,Tailwind CSS 4 主题(light/dark)。

## Monorepo 结构(Turborepo + pnpm workspaces)

```
apps/web      @wafermap/web    Vue3 + Vite + leafer-editor 主应用
apps/server   @wafermap/server FastAPI 位图瓦片服务(Python,.venv 本地构建)
apps/desktop  @wafermap/desktop egui 独立桌面版(Rust,零 JS 依赖)
apps/client   @wafermap/client  Tauri 2 客户端壳(打包 apps/web 的 dist)
```

## 开发

```bash
pnpm install          # 安装全部 workspace 依赖
pnpm dev              # turbo 并行:web (5173) + 瓦片服务 (8000)
pnpm dev:client       # 仅 web(瓦片不可达时前端自动回退本地渲染)
pnpm server           # 仅瓦片服务(独立启动 Python 服务)
pnpm build            # turbo 构建(web: vue-tsc + vite)
pnpm app:dev        # Tauri 客户端(dev,热更新)
pnpm app:install    # 一键:构建 + 安装到 /Applications + 启动(以后 Spotlight 随开)
```

瓦片服务首次使用需在 `apps/server` 构建 venv:

```bash
cd apps/server && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
```
