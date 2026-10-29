<script setup>
/**
 * Global command palette — Spotlight-style search for saved git snippets.
 *
 *   Ctrl+K (or ⌘+K) → opens this panel from anywhere
 *   Type to search (command + note + tag)
 *   ↑↓ to navigate, Enter to copy, Esc to close
 *   Click row to copy; edit/delete via the buttons on each row
 *
 * Mounted globally in App.vue (outside router-view) so it works on any page.
 */
import { ref, computed, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  SearchOutlined, StarOutlined, StarFilled,
  PlusOutlined, EditOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import { useSnippetsStore } from '@/stores/snippets'
import { useRepoStore } from '@/stores/repo'
import SnippetEditor from '@/components/snippets/SnippetEditor.vue'

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const store = useSnippetsStore()
const repoStore = useRepoStore()
const router = useRouter()
const query = ref('')
const activeIdx = ref(0)
const searchInput = ref(null)

// editor modal state
const editorOpen = ref(false)
const editingSnippet = ref(null)

// ensure data is loaded when the palette first opens
watch(
  () => props.open,
  (v) => {
    if (v) {
      if (!store.loaded) store.load()
      query.value = ''
      activeIdx.value = 0
      nextTick(() => searchInput.value?.focus())
    }
  },
)

// filter: match against command + note + tag (case-insensitive)
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return store.snippets
  return store.snippets.filter(
    (s) =>
      s.command.toLowerCase().includes(q) ||
      (s.note || '').toLowerCase().includes(q) ||
      (s.tag || '').toLowerCase().includes(q),
  )
})

// reset selection when results change
watch(filtered, () => { activeIdx.value = 0 })

function close() { emit('update:open', false) }

// resolve template variables in a command: {{branch}} {{repo}} {{remote}}
function resolveTemplate(cmd) {
  if (!cmd || !cmd.includes('{{')) return cmd
  return cmd
    .replace(/\{\{branch\}\}/g, repoStore.branch || 'main')
    .replace(/\{\{repo\}\}/g, repoStore.name || '')
    .replace(/\{\{remote\}\}/g, 'origin')
}

async function copyCommand(s) {
  if (!s?.command) return
  const cmd = resolveTemplate(s.command)
  try {
    await navigator.clipboard.writeText(cmd)
    AMessage.success('已复制', 1.2)
    close()
  } catch {}
}

// Ctrl+Enter: copy + send to terminal (write to PTY + navigate to /terminal)
async function sendToTerminal(s) {
  if (!s?.command) return
  const app = window.go?.main?.ByteGitApp
  const cmd = resolveTemplate(s.command) + '\n' // append Enter to execute
  // base64 encode (UTF-8 safe, same as terminal input)
  const bytes = new TextEncoder().encode(cmd)
  let b64 = ''
  bytes.forEach((b) => { b64 += String.fromCharCode(b) })
  b64 = btoa(b64)
  // try writing to PTY (if terminal is running in keep-alive)
  if (app?.TerminalInput) {
    try { await app.TerminalInput(b64) } catch {}
  }
  // navigate to terminal page
  router.push('/terminal')
  AMessage.success('已发送到终端', 1.2)
  close()
}

// keyboard navigation
function onKeydown(e) {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    activeIdx.value = Math.min(activeIdx.value + 1, filtered.value.length - 1)
    scrollToActive()
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    activeIdx.value = Math.max(activeIdx.value - 1, 0)
    scrollToActive()
  } else if (e.key === 'Enter') {
    e.preventDefault()
    const s = filtered.value[activeIdx.value]
    if (s) {
      if (e.ctrlKey || e.metaKey) sendToTerminal(s)
      else copyCommand(s)
    }
  } else if (e.key === 'Escape') {
    e.preventDefault()
    close()
  }
}

function scrollToActive() {
  nextTick(() => {
    const el = document.querySelector('.cp-row.active')
    el?.scrollIntoView({ block: 'nearest' })
  })
}

// ---- CRUD ----
function openAdd() {
  editingSnippet.value = null
  editorOpen.value = true
}
function openEdit(s) {
  editingSnippet.value = { ...s }
  editorOpen.value = true
}
async function onSave(data) {
  if (editingSnippet.value) {
    // update
    await store.update({ ...editingSnippet.value, ...data })
  } else {
    // add
    await store.add(data)
  }
  editorOpen.value = false
}
function confirmDelete(s) {
  Modal.confirm({
    title: '删除这条命令？',
    content: s.command,
    okText: '删除',
    okType: 'danger',
    cancelText: '取消',
    onOk() { store.remove(s.id) },
  })
}
</script>

<template>
  <teleport to="body">
    <transition name="cp-fade">
      <div v-if="open" class="cp-overlay" @click.self="close">
        <div class="cp-panel">
          <!-- search bar -->
          <div class="cp-search">
            <SearchOutlined class="cp-search-icon" />
            <input
              ref="searchInput"
              v-model="query"
              class="cp-input"
              placeholder="搜索命令…（↑↓ 选择，Enter 复制，Esc 关闭）"
              @keydown="onKeydown"
            />
            <button class="cp-new" @click="openAdd" title="新建命令">
              <PlusOutlined />
            </button>
          </div>

          <!-- results -->
          <div class="cp-results">
            <div v-if="!filtered.length" class="cp-empty">
              {{ query ? '无匹配结果' : '还没有保存的命令，点击 + 新建' }}
            </div>
            <div
              v-for="(s, i) in filtered"
              :key="s.id"
              class="cp-row"
              :class="{ active: i === activeIdx }"
              @click="copyCommand(s)"
              @mouseenter="activeIdx = i"
            >
              <!-- left: command + note -->
              <div class="cp-row-main">
                <div class="cp-cmd">{{ s.command }}</div>
                <div v-if="s.note" class="cp-note">{{ s.note }}</div>
              </div>
              <!-- right: tag + fav + actions -->
              <div class="cp-row-side" @click.stop>
                <span v-if="s.tag" class="cp-tag">{{ s.tag }}</span>
                <button class="cp-icon-btn" @click="store.toggleFav(s.id)" :title="s.favorite ? '取消收藏' : '收藏'">
                  <StarFilled v-if="s.favorite" class="fav-on" />
                  <StarOutlined v-else />
                </button>
                <button class="cp-icon-btn" @click="openEdit(s)" title="编辑">
                  <EditOutlined />
                </button>
                <button class="cp-icon-btn danger" @click="confirmDelete(s)" title="删除">
                  <DeleteOutlined />
                </button>
              </div>
            </div>
          </div>

          <!-- footer hint -->
          <div class="cp-foot">
            <span><kbd>↑</kbd><kbd>↓</kbd> 导航</span>
            <span><kbd>Enter</kbd> 复制</span>
            <span><kbd>Ctrl</kbd>+<kbd>Enter</kbd> 终端执行</span>
            <span><kbd>Esc</kbd> 关闭</span>
            <span class="cp-foot-spacer"></span>
            <span class="cp-count">{{ filtered.length }} 条</span>
          </div>
        </div>

        <!-- editor modal -->
        <a-modal
          v-model:open="editorOpen"
          :title="editingSnippet ? '编辑命令' : '新建命令'"
          width="520px"
          :footer="null"
          destroy-on-close
          @keydown.esc.stop
        >
          <SnippetEditor
            :snippet="editingSnippet"
            @save="onSave"
            @cancel="editorOpen = false"
          />
        </a-modal>
      </div>
    </transition>
  </teleport>
</template>

<style scoped>
/* overlay */
.cp-overlay {
  position: fixed; inset: 0; z-index: 2000;
  display: flex; align-items: flex-start; justify-content: center;
  padding-top: 12vh;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(3px);
}

/* panel */
.cp-panel {
  width: 600px; max-width: 92vw; max-height: 70vh;
  display: flex; flex-direction: column;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

/* search bar */
.cp-search {
  display: flex; align-items: center; gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--color-border-light);
}
.cp-search-icon { font-size: 16px; color: var(--color-text-tertiary); flex-shrink: 0; }
.cp-input {
  flex: 1; border: none; outline: none; background: transparent;
  font-size: 15px; color: var(--color-text);
}
.cp-input::placeholder { color: var(--color-text-tertiary); }
.cp-new {
  display: inline-flex; align-items: center; justify-content: center;
  width: 28px; height: 28px; border: none; border-radius: var(--radius-sm);
  background: var(--color-primary-bg); color: var(--color-primary);
  cursor: pointer; font-size: 14px;
  transition: all var(--transition-fast);
}
.cp-new:hover { background: var(--color-primary); color: #fff; }

/* results */
.cp-results { flex: 1; overflow-y: auto; padding: 6px; }
.cp-empty { padding: 40px 16px; text-align: center; color: var(--color-text-tertiary); font-size: 13px; }

.cp-row {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; border-radius: var(--radius-sm);
  cursor: pointer; transition: background var(--transition-fast);
}
.cp-row:hover, .cp-row.active { background: var(--color-hover); }
.cp-row.active { background: var(--color-primary-bg); }

.cp-row-main { flex: 1; min-width: 0; }
.cp-cmd {
  font-family: 'Cascadia Code', 'JetBrains Mono', monospace;
  font-size: 13px; color: var(--color-text);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.cp-note { font-size: 11px; color: var(--color-text-tertiary); margin-top: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.cp-row-side { display: flex; align-items: center; gap: 2px; flex-shrink: 0; }
.cp-tag {
  font-size: 10px; padding: 1px 6px; border-radius: var(--radius-xs);
  background: var(--color-surface-raised); color: var(--color-text-secondary);
  margin-right: 4px;
}
.cp-icon-btn {
  -webkit-appearance: none;
  appearance: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 24px; height: 24px; min-width: 24px; max-width: 24px;
  padding: 0; margin: 0;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  font-size: 12px;
  line-height: 1;
  box-sizing: border-box;
  transition: all var(--transition-fast);
}
.cp-icon-btn:hover { background: var(--color-hover); color: var(--color-text); }
.cp-icon-btn.danger:hover { color: var(--color-error); }
.fav-on { color: var(--color-warning); }

/* footer */
.cp-foot {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 16px; font-size: 11px; color: var(--color-text-tertiary);
  border-top: 1px solid var(--color-border-light);
}
.cp-foot kbd {
  display: inline-block; padding: 1px 5px;
  border: 1px solid var(--color-border); border-radius: 3px;
  font-size: 10px; font-family: monospace;
  background: var(--color-background); color: var(--color-text-secondary);
  margin-right: 2px;
}
.cp-foot-spacer { flex: 1; }
.cp-count { font-weight: 500; }

/* transition */
.cp-fade-enter-active, .cp-fade-leave-active { transition: opacity var(--transition-fast); }
.cp-fade-enter-from, .cp-fade-leave-to { opacity: 0; }
.cp-fade-enter-active .cp-panel { animation: cp-pop var(--transition-normal); }
@keyframes cp-pop {
  from { transform: scale(0.96) translateY(-8px); opacity: 0; }
  to { transform: scale(1) translateY(0); opacity: 1; }
}
</style>
