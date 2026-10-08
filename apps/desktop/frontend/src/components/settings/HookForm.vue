<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  ThunderboltOutlined, DeleteOutlined, SaveOutlined, PlayCircleOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '../common/CodeViewer.vue'

/**
 * Hook 管理 — 列表 + CodeMirror 编辑器 + 安装/卸载/测试运行。
 */
const app = window.go?.main?.ByteGitApp

const hooks = ref([])
const loading = ref(false)
const activeHook = ref('')
const script = ref('')
const scriptLoading = ref(false)
const saving = ref(false)
const testing = ref(false)
const dirty = ref(false)

const defaultScript = `-- hook 脚本（Lua）
-- 定义 main 函数，返回 0 = 放行，非 0 = 中止 git 操作
-- 可用 os.getenv() 读取 git 传入的环境变量
function main(hook_name)
  print("hook " .. hook_name .. " triggered")
  return 0
end
`

const activeHookInfo = computed(() =>
  hooks.value.find((h) => h.name === activeHook.value) || null,
)

async function loadHooks() {
  if (!app?.HookList) return
  loading.value = true
  try {
    hooks.value = await app.HookList()
    if (hooks.value.length && !activeHook.value) {
      activeHook.value = hooks.value[0].name
      await loadScript()
    }
  } catch { hooks.value = [] }
  finally { loading.value = false }
}

async function loadScript() {
  if (!activeHook.value || !app?.HookReadScript) return
  scriptLoading.value = true
  try {
    const content = await app.HookReadScript(activeHook.value)
    script.value = content || defaultScript
    dirty.value = false
  } catch { script.value = defaultScript }
  finally { scriptLoading.value = false }
}

function selectHook(name) {
  if (dirty.value && !confirm('当前脚本未保存，切换将丢弃改动，确定？')) return
  activeHook.value = name
  loadScript()
}

function onScriptChange(v) {
  script.value = v
  dirty.value = true
}

async function save() {
  if (!activeHook.value || !app?.HookInstallLua) return
  saving.value = true
  try {
    await app.HookInstallLua(activeHook.value, script.value)
    AMessage.success(`已安装 hook: ${activeHook.value}`)
    dirty.value = false
    await loadHooks()
  } catch (e) {
    AMessage.error('安装失败: ' + String(e?.message || e))
  } finally {
    saving.value = false
  }
}

function confirmRemove() {
  if (!activeHook.value || !activeHookInfo.value?.enabled) return
  Modal.confirm({
    title: `卸载 hook「${activeHook.value}」？`,
    content: '将删除 .git/hooks/ 下的脚本文件和配置记录。',
    okText: '卸载', okType: 'danger', cancelText: '取消',
    async onOk() {
      try {
        await app.HookRemove(activeHook.value)
        AMessage.success('已卸载')
        script.value = defaultScript
        dirty.value = false
        await loadHooks()
      } catch (e) { AMessage.error('卸载失败: ' + String(e?.message || e)) }
    },
  })
}

async function testRun() {
  if (!activeHook.value || !app?.HookExec) return
  testing.value = true
  try {
    const result = await app.HookExec(activeHook.value)
    if (result.exitCode === 0) {
      AMessage.success(result.output || '执行成功')
    } else {
      AMessage.error(result.error || result.output || `执行失败 (exit ${result.exitCode})`)
    }
  } catch (e) {
    AMessage.error('执行失败: ' + String(e?.message || e))
  } finally {
    testing.value = false
  }
}

const statusBadge = {
  lua: { text: 'Lua', color: 'blue' },
  script: { text: 'Script', color: 'green' },
  sample: { text: 'Sample', color: 'default' },
  '': { text: '未安装', color: 'default' },
}

onMounted(loadHooks)
</script>

<template>
  <div class="hk-form">
    <div v-if="loading" class="hk-center"><a-spin /></div>
    <div v-else class="hk-layout">
        <!-- left: hook list -->
        <aside class="hk-list">
          <div class="hk-list-head">Hook 槽位（{{ hooks.length }}）</div>
          <div class="hk-list-body">
            <div
              v-for="h in hooks" :key="h.name"
              class="hk-item" :class="{ active: h.name === activeHook }"
              @click="selectHook(h.name)"
            >
              <span class="hk-name">{{ h.name }}</span>
              <span class="hk-badge" :class="'hb-' + (h.type || 'none')">
                {{ statusBadge[h.type || '']?.text || '—' }}
              </span>
            </div>
          </div>
        </aside>

        <!-- right: editor -->
        <section class="hk-editor-area">
          <div class="hk-editor-head">
            <span class="hk-active-name">{{ activeHook || '—' }}</span>
            <span v-if="dirty" class="hk-dirty">未保存 ●</span>
            <div class="hk-actions">
              <a-button size="small" type="primary" :loading="saving" :disabled="!activeHook" @click="save">
                <template #icon><SaveOutlined /></template>保存并安装
              </a-button>
              <a-button size="small" :loading="testing" :disabled="!activeHook || !activeHookInfo?.enabled" @click="testRun">
                <template #icon><PlayCircleOutlined /></template>测试
              </a-button>
              <a-button size="small" danger :disabled="!activeHookInfo?.enabled" @click="confirmRemove">
                <template #icon><DeleteOutlined /></template>卸载
              </a-button>
            </div>
          </div>
          <div class="hk-editor">
            <a-spin v-if="scriptLoading" class="hk-spin" />
            <CodeViewer
              v-else
              :code="script"
              filename="hook.lua"
              editable
              @update:modelValue="onScriptChange"
            />
          </div>
          <div class="hk-hint">
            <ThunderboltOutlined /> 定义 <code>function main(hook_name)</code>，返回 0 放行 / 非 0 中止。
            可用 <code>os.getenv()</code> 读取 git 环境变量。
          </div>
        </section>
      </div>
  </div>
</template>

<style scoped>
.hk-form { flex: 1; display: flex; flex-direction: column; padding: 20px 24px; background: var(--color-background); overflow: hidden; min-height: 0; }
.hk-center { flex: 1; display: flex; align-items: center; justify-content: center; }
.hk-layout { flex: 1; display: flex; gap: var(--spacing-md); min-height: 0; }

/* left: hook list */
.hk-list { width: 200px; flex-shrink: 0; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); display: flex; flex-direction: column; overflow: hidden; }
.hk-list-head { padding: 8px 12px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.hk-list-body { flex: 1; overflow-y: auto; padding: 4px; }
.hk-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 10px; border-radius: var(--radius-xs); cursor: pointer;
  font-size: 12px; color: var(--color-text-secondary);
  transition: all var(--transition-fast);
}
.hk-item:hover { background: var(--color-hover); }
.hk-item.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 500; }
.hk-badge { font-size: 9px; padding: 1px 5px; border-radius: 3px; font-weight: 600; flex-shrink: 0; }
.hb-lua { background: var(--color-info-bg); color: var(--color-info); }
.hb-script { background: var(--color-success-bg); color: var(--color-success); }
.hb-sample, .hb-none { background: var(--color-surface-raised); color: var(--color-text-tertiary); }

/* right: editor */
.hk-editor-area { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
.hk-editor-head {
  display: flex; align-items: center; gap: 8px; padding: 8px 12px;
  border-bottom: 1px solid var(--color-border); flex-shrink: 0;
}
.hk-active-name { font-family: 'Cascadia Code', monospace; font-size: 13px; color: var(--color-text); font-weight: 500; }
.hk-dirty { font-size: 11px; color: var(--color-warning); font-weight: 600; }
.hk-actions { margin-left: auto; display: flex; gap: 6px; }
.hk-editor { flex: 1; min-height: 0; position: relative; overflow: hidden; }
.hk-spin { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
.hk-hint {
  padding: 8px 12px; font-size: 11px; color: var(--color-text-tertiary);
  border-top: 1px solid var(--color-border-light); flex-shrink: 0;
  display: flex; align-items: center; gap: 4px;
}
.hk-hint code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }
</style>
