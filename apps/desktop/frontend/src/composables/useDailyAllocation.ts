/**
 * 逐日分配算法集 —— 核心不变量：任何生成器/补偿器输出 Σcount === total。
 *
 * 供 DailyDistributeModal（改写页独立弹窗）与 DateRemapPanel（导出向导
 * 逐日模式）共用，替代此前两处各自的 ceil 均分 + 静默修正（打开即超出、
 * 行为不可预期）。
 *
 * 全部为纯函数（随机源可注入以便测试）。
 */

/** 每日条数形状策略。 */
export type ShapeStrategy = 'even' | 'wave' | 'weekday'
/** 手动编辑某天后，差额在其他行上的补偿方向。 */
export type CompensateStrategy = 'proportional' | 'tail' | 'head'

export const SHAPE_LABELS: Record<ShapeStrategy, string> = {
  even: '均匀',
  wave: '随机波动',
  weekday: '工作日优先',
}

export const COMPENSATE_LABELS: Record<CompensateStrategy, string> = {
  proportional: '按比例',
  tail: '从尾部',
  head: '从头部',
}

/** 简单可注入随机源（默认 Math.random）。 */
export type Rand = () => number

/** 均匀分配：最大余数法——前 total % days 天各多 1，合计精确等于 total。 */
export function balancedEven(total: number, days: number): number[] {
  if (days <= 0) return []
  const t = Math.max(0, Math.floor(total))
  const base = Math.floor(t / days)
  const rem = t % days
  return Array.from({ length: days }, (_, i) => base + (i < rem ? 1 : 0))
}

/** 随机波动：各天在 ±pct 区间随机取整，残差逐日修补保持精确合计。 */
export function balancedWave(total: number, days: number, pct = 0.35, rand: Rand = Math.random): number[] {
  if (days <= 0) return []
  const t = Math.max(0, Math.floor(total))
  if (t === 0) return Array.from({ length: days }, () => 0)
  const base = t / days
  const out = Array.from({ length: days }, () => {
    const factor = 1 + (rand() * 2 - 1) * pct
    return Math.max(0, Math.round(base * factor))
  })
  return fixResidual(out, t)
}

/** 工作日权重：周一~五权重 1、周末 weight（默认 0.25），最大余数法补齐。 */
export function balancedWeekday(
  total: number,
  dates: { day: () => number }[],
  weekendWeight = 0.25,
): number[] {
  if (dates.length <= 0) return []
  const t = Math.max(0, Math.floor(total))
  // dayjs day(): 0=周日 … 6=周六
  const weights = dates.map((d) => (d.day() === 0 || d.day() === 6 ? weekendWeight : 1))
  return weightedExact(t, weights)
}

/**
 * 差额补偿：把 counts 修正到精确 total（手动编辑后的自动再平衡）。
 * skip 索引的行不参与补偿（通常是用户刚编辑的那行）。
 */
export function rebalance(
  counts: number[],
  total: number,
  strategy: CompensateStrategy = 'proportional',
  skip: number | null = null,
): number[] {
  const out = counts.map((c) => Math.max(0, Math.floor(c || 0)))
  const t = Math.max(0, Math.floor(total))
  const idx = orderFor(strategy, out, skip)
  let cur = out.reduce((s, c) => s + c, 0)
  if (cur > t) {
    // 超出：按策略顺序逐行扣减（被 skip 的行最后才动，尽量不动）
    for (const i of orderFor(strategy === 'proportional' ? 'tail' : strategy, out, skip)) {
      if (cur <= t) break
      const cut = Math.min(out[i], cur - t)
      out[i] -= cut
      cur -= cut
    }
  } else if (cur < t) {
    // 不足：按策略顺序补足；比例模式按现有占比摊、残差给最后一行
    if (strategy === 'proportional') {
      const remain = t - cur
      const movable = idx.filter((i) => i !== skip)
      const base = movable.reduce((s, i) => s + out[i], 0)
      let allocated = 0
      const order = movable.slice()
      for (let k = 0; k < order.length; k++) {
        const i = order[k]
        if (k === order.length - 1) {
          out[i] += remain - allocated
        } else {
          const add = base > 0 ? Math.floor((remain * out[i]) / base) : Math.floor(remain / order.length)
          out[i] += add
          allocated += add
        }
      }
      if (!order.length && skip != null) out[skip] += remain
    } else {
      const remain = t - cur
      for (const i of idx) {
        if (remain <= 0) break
        // 尾部/头部策略：整额补给该行
        out[i] += remain
        break
      }
    }
  }
  return out
}

/**
 * 手动编辑某一行为 newCount 后，把差额摊到其他行，返回保持 Σ===total 的新数组。
 * 编辑行的新值尽量保留（仅在其余行容量不足/溢出时才回压）。
 */
export function compensateAfterEdit(
  counts: number[],
  total: number,
  editedIndex: number,
  newCount: number,
  strategy: CompensateStrategy = 'proportional',
): number[] {
  const t = Math.max(0, Math.floor(total))
  const next = counts.map((c, i) => (i === editedIndex ? Math.max(0, Math.floor(newCount || 0)) : c))
  // 先固定编辑行，把其余行修正到 total - edited
  const rest = rebalance(
    next.filter((_, i) => i !== editedIndex),
    t - next[editedIndex],
    strategy,
    null,
  )
  const out = next.slice()
  let k = 0
  for (let i = 0; i < out.length; i++) {
    if (i !== editedIndex) out[i] = rest[k++] ?? 0
  }
  // 其余行全为 0 仍不足（如只编辑了一行且调小过多）：回压到编辑行
  const sum = out.reduce((s, c) => s + c, 0)
  if (sum !== t) out[editedIndex] += t - sum
  return out.map((c) => Math.max(0, c))
}

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

/** 按权重精确分配（最大余数法）：Σout === total。 */
function weightedExact(total: number, weights: number[]): number[] {
  const sum = weights.reduce((s, w) => s + w, 0)
  if (sum <= 0 || total <= 0) return weights.map(() => 0)
  const raw = weights.map((w) => (total * w) / sum)
  const out = raw.map((r) => Math.floor(r))
  return fixResidual(out, total)
}

/** 把 Σout 修补到 exact：逐单位加/减到（伪）随机的行上，避免全部堆在一行。 */
function fixResidual(counts: number[], exact: number, rand: Rand = Math.random): number[] {
  const out = counts.slice()
  let diff = exact - out.reduce((s, c) => s + c, 0)
  while (diff > 0) {
    const i = Math.floor(rand() * out.length)
    out[i] += 1
    diff -= 1
  }
  while (diff < 0) {
    // 从随机行扣，跳过已经是 0 的行
    const i = Math.floor(rand() * out.length)
    if (out[i] > 0) {
      out[i] -= 1
      diff += 1
    }
    // 全 0 保护：极端情况下强制退出
    if (out.every((c) => c === 0)) break
  }
  return out
}

/** 补偿顺序：proportional/tail 从尾部往前，head 从头部往后。 */
function orderFor(strategy: CompensateStrategy, counts: number[], skip: number | null): number[] {
  let idx = counts.map((_, i) => i).filter((i) => i !== skip)
  if (strategy !== 'head') idx = idx.reverse()
  return idx
}
