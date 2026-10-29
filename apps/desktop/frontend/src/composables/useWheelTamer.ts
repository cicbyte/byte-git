/**
 * useWheelTamer — 全局滚轮速度归一。
 *
 * WebView2 的原生滚轮在部分鼠标/驱动组合下每档位移过大（"滚一下飞一片"）。
 * 这里统一缩放并封顶单次位移：
 *   - deltaY / deltaX 一律 × factor，且单次位移封顶 maxStep（自由滚轮回飞也不会一屏）；
 *   - ctrl + 滚轮（缩放手势）与已被其它组件阻止的事件不处理；
 *   - antd 弹层（下拉/日期/级联/悬浮提示）豁免——它们是自绘 div+虚拟滚动，
 *     antd 自己处理 wheel，叠加归一会双重滚动且丢失原生平滑手感；
 *   - 沿祖先链找「能吃下当前方向位移」的最近滚动容器——内层容器滚到头
 *     自动带动外层（复刻 Chromium 默认滚动链语义）；
 *   - 找不到可滚容器时保持默认行为（页面整体无滚动时不拦截）。
 */

const WHEEL_FACTOR = 0.65
const WHEEL_MAX_STEP = 90

/** antd 弹层层（挂在 body 的 portal）——保持原生滚动手感。 */
const OVERLAY_SELECTOR =
  '.ant-select-dropdown, .ant-dropdown, .ant-picker-dropdown, .ant-cascader-menu, .ant-tooltip'

export function installWheelTamer(
  factor: number = WHEEL_FACTOR,
  maxStep: number = WHEEL_MAX_STEP,
): () => void {
  const isScrollable = (el: Element): boolean => {
    const cs = getComputedStyle(el)
    return /(auto|scroll|overlay)/.test(cs.overflowY) && el.scrollHeight > el.clientHeight + 1
  }

  const canConsume = (el: Element, dir: number): boolean =>
    dir < 0 ? el.scrollTop > 0 : el.scrollTop + el.clientHeight < el.scrollHeight - 1

  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey || e.defaultPrevented || (e.deltaY === 0 && e.deltaX === 0)) return
    // antd 弹层豁免：恢复原生滚动（antd 虚拟列表自带 wheel 处理）
    const src = e.target
    if (src instanceof Element && src.closest(OVERLAY_SELECTOR)) return

    const dir = Math.sign(e.deltaY)
    let target: Element | null = null
    let node = e.target as Element | null
    while (node && node !== document.documentElement) {
      if (node instanceof Element && isScrollable(node)) {
        if (dir === 0 || canConsume(node, dir)) { target = node; break }
        // 内层已滚到头：继续向外层找（滚动链）
      }
      node = node.parentElement
    }
    if (!target) {
      const doc = document.scrollingElement
      if (doc instanceof Element && isScrollable(doc)) target = doc
    }
    if (!target) return

    e.preventDefault()
    const scale = (d: number) => Math.sign(d) * Math.min(Math.abs(d) * factor, maxStep)
    target.scrollBy({ top: scale(e.deltaY), left: scale(e.deltaX) })
  }

  window.addEventListener('wheel', onWheel, { passive: false })
  return () => window.removeEventListener('wheel', onWheel)
}
