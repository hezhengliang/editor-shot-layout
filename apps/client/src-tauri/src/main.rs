// Tauri 客户端壳:加载打包后的 web 端(dist),行为与浏览器版一致。
// BitMap 瓦片服务为外部依赖(localhost:8000),不可达时前端自动回退本地渲染。
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
