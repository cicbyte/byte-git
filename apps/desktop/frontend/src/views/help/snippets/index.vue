<script setup>
/**
 * 命令片段管理页面 — 列表 + CRUD + 搜索。
 * 全局快捷搜索走 Ctrl+K（CommandPalette），此页面用于完整管理。
 */
import { ref, computed, onMounted } from 'vue'
import { Modal } from 'ant-design-vue'
import {
  PlusOutlined, EditOutlined, DeleteOutlined, StarOutlined, StarFilled,
  SearchOutlined, SnippetsOutlined, CopyOutlined,
} from '@ant-design/icons-vue'
import { useSnippetsStore } from '@/stores/snippets'
import { usePageFooter } from '@/composables/usePageFooter'
import { message as AMessage } from 'ant-design-vue'
import SnippetEditor from '@/components/snippets/SnippetEditor.vue'

const store = useSnippetsStore()
const { setFooter, clearFooter } = usePageFooter()

const search = ref('')
const editorOpen = ref(false)
const editing = ref(null)
const activeTag = ref('')

const filtered = computed(() => {
  let list = store.snippets
  if (activeTag.value) list = list.filter((s) => s.tag === activeTag.value)
  const q = search.value.trim().toLowerCase()
  if (q) {
    list = list.filter(
      (s) =>
        s.command.toLowerCase().includes(q) ||
        (s.note || '').toLowerCase().includes(q),
    )
  }
  return list
})

onMounted(async () => {
  await store.load()
  setFooter(SnippetsFooter, () => ({
    count: store.snippets.length,
    tags: store.tags.length,
  }))
})
import { h } from 'vue'
const SnippetsFooter = {
  props: ['count', 'tags'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(SnippetsOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, '命令片段'),
      h('span', `${this.count} 条 · ${this.tags} 个分类`),
      h('div', { style: 'flex:1' }),
      h('span', 'Ctrl+K 快捷搜索'),
    ])
  },
}
import { onBeforeUnmount } from 'vue'
onBeforeUnmount(() => clearFooter())

function openAdd() { editing.value = null; editorOpen.value = true }
function openEdit(s) { editing.value = { ...s }; editorOpen.value = true }
async function onSave(data) {
  if (editing.value) await store.update({ ...editing.value, ...data })
  else await store.add(data)
  editorOpen.value = false
}
function confirmDelete(s) {
  Modal.confirm({
    title: '删除这条命令？', content: s.command,
    okText: '删除', okType: 'danger', cancelText: '取消',
    onOk() { store.remove(s.id) },
  })
}
async function copy(s) {
  try { await navigator.clipboard.writeText(s.command); AMessage.success('已复制', 1.2) } catch {}
}
</script>

<template>
  <div class="sv-view">
    <!-- toolbar -->
    <div class="sv-toolbar">
      <a-input
        v-model:value="search"
        placeholder="搜索命令…"
        allow-clear
        size="small"
        class="sv-search"
      >
        <template #prefix><SearchOutlined /></template>
      </a-input>
      <!-- tag filter -->
      <div class="sv-tags">
        <button class="sv-tag-btn" :class="{ active: !activeTag }" @click="activeTag = ''">全部</button>
        <button
          v-for="t in store.tags" :key="t"
          class="sv-tag-btn" :class="{ active: activeTag === t }"
          @click="activeTag = t"
        >{{ t }}</button>
      </div>
      <a-button size="small" @click="openAdd">
        <template #icon><PlusOutlined /></template>新建
      </a-button>
    </div>

    <!-- list -->
    <div class="sv-list">
      <div v-if="!filtered.length" class="sv-empty">
        <SnippetsOutlined style="font-size:36px;opacity:0.3" />
        <div style="margin-top:8px">{{ search || activeTag ? '无匹配结果' : '还没有保存的命令，点击「新建」' }}</div>
      </div>
      <div v-for="s in filtered" :key="s.id" class="sv-card" :class="{ fav: s.favorite }">
        <div class="sv-card-main" @click="copy(s)">
          <div class="sv-cmd-row">
            <span class="sv-cmd">{{ s.command }}</span>
            <span v-if="s.tag" class="sv-card-tag">{{ s.tag }}</span>
          </div>
          <div v-if="s.note" class="sv-note">{{ s.note }}</div>
        </div>
        <div class="sv-card-actions">
          <button class="sv-act" @click="copy(s)" title="复制"><CopyOutlined /></button>
          <button class="sv-act" @click="store.toggleFav(s.id)" :title="s.favorite ? '取消收藏' : '收藏'">
            <StarFilled v-if="s.favorite" class="fav-on" />
            <StarOutlined v-else />
          </button>
          <button class="sv-act" @click="openEdit(s)" title="编辑"><EditOutlined /></button>
          <button class="sv-act danger" @click="confirmDelete(s)" title="删除"><DeleteOutlined /></button>
        </div>
      </div>
    </div>

    <!-- editor modal -->
    <a-modal v-model:open="editorOpen" :title="editing ? '编辑命令' : '新建命令'" width="520px" :footer="null" destroy-on-close>
      <SnippetEditor :snippet="editing" @save="onSave" @cancel="editorOpen = false" />
    </a-modal>
  </div>
</template>

<style scoped>
.sv-view { width: 100%; height: 100%; display: flex; flex-direction: column; gap: var(--spacing-sm); background: var(--color-background); padding: var(--spacing-md); overflow: hidden; }

.sv-toolbar { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
.sv-search { width: 240px; }
.sv-tags { display: flex; gap: 4px; flex: 1; overflow-x: auto; }
.sv-tag-btn {
  padding: 4px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-xs);
  background: var(--color-surface); color: var(--color-text-secondary);
  font-size: 12px; cursor: pointer; white-space: nowrap;
  transition: all var(--transition-fast);
}
.sv-tag-btn:hover { background: var(--color-hover); }
.sv-tag-btn.active { background: var(--color-primary-bg); color: var(--color-primary); border-color: var(--color-primary); }

.sv-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; }
.sv-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }

.sv-card {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  transition: all var(--transition-fast);
}
.sv-card:hover { border-color: var(--color-primary); box-shadow: var(--shadow-xs); }
.sv-card.fav { border-left: 3px solid var(--color-warning); }

.sv-card-main { flex: 1; min-width: 0; cursor: pointer; }
.sv-cmd-row { display: flex; align-items: center; gap: 8px; }
.sv-cmd {
  font-family: 'Cascadia Code', 'JetBrains Mono', monospace;
  font-size: 13px; color: var(--color-text);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.sv-card-tag { font-size: 10px; padding: 1px 6px; border-radius: var(--radius-xs); background: var(--color-surface-raised); color: var(--color-text-secondary); flex-shrink: 0; }
.sv-note { font-size: 12px; color: var(--color-text-tertiary); margin-top: 3px; }

.sv-card-actions { display: flex; gap: 2px; flex-shrink: 0; }
.sv-act {
  -webkit-appearance: none; appearance: none;
  display: inline-flex; align-items: center; justify-content: center;
  flex-shrink: 0;
  width: 28px; height: 28px; min-width: 28px; max-width: 28px;
  padding: 0; margin: 0;
  border: none; border-radius: var(--radius-xs);
  background: transparent; color: var(--color-text-tertiary);
  cursor: pointer; font-size: 13px; line-height: 1;
  box-sizing: border-box; transition: all var(--transition-fast);
}
.sv-act:hover { background: var(--color-hover); color: var(--color-text); }
.sv-act.danger:hover { color: var(--color-error); }
.fav-on { color: var(--color-warning); }
</style>
