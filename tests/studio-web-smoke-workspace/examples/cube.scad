// 用于 studio-web Phase 6 smoke 的最小 cube 预览样例。
// web 端不允许通过 FileRead 读取 .scad 源码（保密边界），split viewer
// 源码面会显示拒绝消息；预览面走 PreviewRequest，由 server 处理。
cube([10, 10, 10]);
