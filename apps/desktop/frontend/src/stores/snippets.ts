import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { message as AMessage } from 'ant-design-vue'

/**
 * Snippet store — personal git command library.
 *
 * Backed by ~/.cicbyte/apps/byte-git/snippets.json (Go side). The store loads once
 * and mutates optimistically; every add/update/delete/toggle calls the
 * backend to persist.
 */
export interface Snippet {
  id: string
  command: string
  note: string
  tag: string
  favorite: boolean
  createdAt: number
}

type App = {
  SnippetsList(): Promise<Snippet[]>
  SnippetsAdd(s: Partial<Snippet>): Promise<Snippet>
  SnippetsUpdate(s: Snippet): Promise<void>
  SnippetsDelete(id: string): Promise<void>
  SnippetsToggleFavorite(id: string): Promise<void>
  SnippetsFilePath(): string
}

const getApp = (): App | undefined => (window as any).go?.main?.ByteGitApp

function errMsg(e: unknown): string {
  return String((e as any)?.message || e)
}

export const useSnippetsStore = defineStore('snippets', () => {
  const snippets = ref<Snippet[]>([])
  const loaded = ref(false)

  /** all unique tags extracted from snippets (for the editor's dropdown) */
  const tags = computed(() => {
    const set = new Set<string>()
    snippets.value.forEach((s) => { if (s.tag) set.add(s.tag) })
    return [...set].sort()
  })

  async function load() {
    const app = getApp()
    if (!app) return
    try {
      snippets.value = await app.SnippetsList()
      loaded.value = true
    } catch (e) {
      AMessage.error('加载命令片段失败: ' + errMsg(e))
    }
  }

  async function add(s: Partial<Snippet>): Promise<Snippet | null> {
    const app = getApp()
    if (!app) return null
    try {
      const created = await app.SnippetsAdd(s)
      snippets.value.push(created)
      return created
    } catch (e) {
      AMessage.error('新增失败: ' + errMsg(e))
      return null
    }
  }

  async function update(s: Snippet) {
    const app = getApp()
    if (!app) return
    try {
      await app.SnippetsUpdate(s)
      const i = snippets.value.findIndex((x) => x.id === s.id)
      if (i >= 0) snippets.value[i] = { ...s }
    } catch (e) {
      AMessage.error('更新失败: ' + errMsg(e))
    }
  }

  async function remove(id: string) {
    const app = getApp()
    if (!app) return
    try {
      await app.SnippetsDelete(id)
      snippets.value = snippets.value.filter((x) => x.id !== id)
    } catch (e) {
      AMessage.error('删除失败: ' + errMsg(e))
    }
  }

  async function toggleFav(id: string) {
    const app = getApp()
    if (!app) return
    try {
      await app.SnippetsToggleFavorite(id)
      const s = snippets.value.find((x) => x.id === id)
      if (s) s.favorite = !s.favorite
    } catch (e) {
      AMessage.error('操作失败: ' + errMsg(e))
    }
  }

  return { snippets, loaded, tags, load, add, update, remove, toggleFav }
})
