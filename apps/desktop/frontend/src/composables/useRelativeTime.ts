/**
 * Format a date string into a Chinese relative-time label, e.g. "3天前".
 *
 * Shared by MetaRewriteView / AIRewriteView (and available to any commit list).
 * Pure function — no reactive state — so it's a plain export rather than a
 * factory, unlike most composables in this folder.
 *
 * Accepts the `-`-separated dates emitted by the Go backend; replaces them
 * with `/` so that legacy WebKit parses the value reliably across platforms.
 */
export function relTime(dateStr: string): string {
  if (!dateStr) return ''
  const t = new Date(dateStr.replace(/-/g, '/'))
  if (isNaN(t.getTime())) return ''
  const diff = Date.now() - t.getTime()
  const m = Math.floor(diff / 60000)
  if (m < 1) return '刚刚'
  if (m < 60) return m + '分钟前'
  const h = Math.floor(m / 60)
  if (h < 24) return h + '小时前'
  const d = Math.floor(h / 24)
  if (d < 30) return d + '天前'
  const mo = Math.floor(d / 30)
  if (mo < 12) return mo + '个月前'
  return Math.floor(mo / 12) + '年前'
}
