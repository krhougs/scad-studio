// ownership §3 唯一允许的 RequestId → resolver 表。
// 约束：值只能是 { resolve, reject } 闭包，禁止缓存任何业务载荷 / workspace 数据 /
// preview 状态。事件派发到 resolver 后必须立即 delete。

export type RequestResolver<T> = {
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
};

export class RequestResolverMap {
  private readonly table = new Map<bigint, RequestResolver<unknown>>();

  register<T>(requestId: bigint, resolver: RequestResolver<T>): void {
    this.table.set(requestId, resolver as RequestResolver<unknown>);
  }

  resolve(requestId: bigint, value: unknown): boolean {
    const entry = this.table.get(requestId);
    if (!entry) return false;
    this.table.delete(requestId);
    entry.resolve(value);
    return true;
  }

  reject(requestId: bigint, error: unknown): boolean {
    const entry = this.table.get(requestId);
    if (!entry) return false;
    this.table.delete(requestId);
    entry.reject(error);
    return true;
  }

  clear(error: unknown): void {
    for (const [, entry] of this.table) {
      entry.reject(error);
    }
    this.table.clear();
  }

  size(): number {
    return this.table.size;
  }
}
