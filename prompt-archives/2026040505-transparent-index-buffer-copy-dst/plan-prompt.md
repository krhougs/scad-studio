## 背景

- 用户运行程序后遇到 `wgpu` 验证错误：
  - `mesh_transparent_index_buffer` 的 usage 只有 `INDEX`
  - 但 `Queue::write_buffer` 需要 `COPY_DST`
- 这是上一轮“透明三角面排序”接入后的直接运行时错误，根因明确，无需继续猜测。

## 目标

- 为会被每帧更新的透明索引缓冲补上 `COPY_DST`。
- 用最小测试约束这条 usage 规则，避免后续回归。
