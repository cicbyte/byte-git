import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { useRepoStore } from './repo'

/** AI 改写任务（左树/跨页入口加入的提交条目 + 生成结果与采用状态） */
export interface AiTask {
  hash: string
  original: string
  author?: string
  date?: string
  pushed?: boolean
  needAi?: boolean
  suggested?: string
  /** pending | suggested | approved | retry | skipped */
  status?: string
}

/**
 * AI 改写任务队列 — 全局态（跨路由保留，切页面再回来不丢）。
 *
 * 仓库守卫：repoEpoch 变化（切换/重开仓库）时自动清空——旧仓库的任务
 * hash 在新仓库无意义，保留会有跨仓库误应用的风险。
 */
export const useAiRewriteStore = defineStore('aiRewrite', () => {
  const repo = useRepoStore()
  const tasks = ref<AiTask[]>([])

  /** 加入任务（按 hash 去重，拷贝隔离来源数据） */
  function addEntry(entry: AiTask): boolean {
    if (!entry?.hash || tasks.value.some((t) => t.hash === entry.hash)) return false
    tasks.value.push({
      ...entry,
      suggested: entry.suggested || '',
      status: entry.status || 'pending',
    })
    return true
  }

  /** 批量加入，返回实际新增数 */
  function addEntries(list: AiTask[]): number {
    let n = 0
    for (const e of list || []) if (addEntry(e)) n++
    return n
  }

  function removeAt(idx: number) {
    tasks.value.splice(idx, 1)
  }

  function clear() {
    tasks.value = []
  }

  watch(() => repo.repoEpoch, clear)

  return { tasks, addEntry, addEntries, removeAt, clear }
})
