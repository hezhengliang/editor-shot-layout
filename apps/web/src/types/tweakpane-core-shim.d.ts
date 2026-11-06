// tweakpane@4 的 .d.ts 引用了未发布的 @tweakpane/core@4(仅本地 file: 依赖),
// 导致 Pane 从 FolderApi 继承的成员类型解析失败。
// 运行时 tweakpane 已打包全部代码,这里用模块增强补全常用 API 签名。
export {}

declare module 'tweakpane' {
  interface Pane {
    addFolder(params: { title: string; expanded?: boolean; index?: number }): any
    addBinding(target: any, key: string, options?: any): any
    addButton(params: { title: string }): any
    addSeparator(options?: any): any
  }
}
