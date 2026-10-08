<script setup>
/**
 * 分类规则管理弹窗 — 从 IgnoreView 拆出。
 * 左侧分类 tab + 右侧关键词编辑，支持增删改、右键菜单。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { PlusOutlined, ReloadOutlined } from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const catRules = ref([])
const catActiveIdx = ref(0)
const catCtxOpen = ref(false)
const catCtxIdx = ref(-1)

function catCtxDelete() {
  catCtxOpen.value = false
  if (catCtxIdx.value < 0) return
  catRules.value.splice(catCtxIdx.value, 1)
  if (catActiveIdx.value >= catRules.value.length) {
    catActiveIdx.value = Math.max(0, catRules.value.length - 1)
  }
}

async function openCategories() {
  catActiveIdx.value = 0
  try {
    catRules.value = await app.IgnoreLoadCategories()
  } catch { catRules.value = [] }
}

function addCategory() {
  catRules.value.push({ name: '新分类', keywords: [] })
}

function removeCategory(idx) {
  catRules.value.splice(idx, 1)
}

async function saveCategories() {
  try {
    await app.IgnoreSaveCategories(JSON.parse(JSON.stringify(catRules.value)))
    AMessage.success('分类规则已保存')
    emit('update:open', false)
  } catch (e) {
    AMessage.error('保存失败: ' + String(e?.message || e))
  }
}

async function resetCategories() {
  try {
    catRules.value = await app.IgnoreResetCategories()
    AMessage.success('已重置为默认')
  } catch (e) {
    AMessage.error('重置失败: ' + String(e?.message || e))
  }
}

watch(() => props.open, (v) => { if (v) openCategories() })
</script>

<template>
  <a-modal :open="open" @update:open="(v) => emit('update:open', v)"
    title="分类规则" width="680px" :footer="null" destroy-on-close>
    <div class="cat-modal-body">
      <div class="cat-layout">
        <!-- left: categories -->
        <aside class="cat-sidebar">
          <div
            v-for="(cat, idx) in catRules" :key="idx"
            class="cat-tab" :class="{ active: catActiveIdx === idx }"
            @click="catActiveIdx = idx"
            @contextmenu.prevent="catCtxIdx = idx; catCtxOpen = true"
          >
            <span class="cat-tab-name">{{ cat.name }}</span>
            <span class="cat-tab-count">{{ cat.keywords.length }}</span>
          </div>
          <a-button size="small" block @click="addCategory(); catActiveIdx = catRules.length - 1">
            <PlusOutlined /> 添加分类
          </a-button>
        </aside>
        <!-- right: editor -->
        <section v-if="catRules[catActiveIdx]" class="cat-editor">
          <div class="cat-editor-head">
            <a-input v-model:value="catRules[catActiveIdx].name" size="small" style="width: 160px" />
            <span class="cat-editor-count">{{ catRules[catActiveIdx].keywords.length }} 个关键词</span>
          </div>
          <a-textarea
            :value="catRules[catActiveIdx].keywords.join('\n')"
            @update:value="(v) => catRules[catActiveIdx].keywords = v.split('\n').filter(k => k.trim())"
            :auto-size="{ minRows: 12, maxRows: 20 }"
            class="cat-editor-area"
            placeholder="每行一个关键词（支持 glob）"
          />
        </section>
      </div>
      <div class="cat-footer">
        <a-button size="small" @click="resetCategories" title="恢复默认">
          <template #icon><ReloadOutlined /></template>恢复默认
        </a-button>
        <div class="cat-footer-spacer"></div>
        <a-button size="small" @click="emit('update:open', false)">取消</a-button>
        <a-button size="small" type="primary" @click="saveCategories">保存</a-button>
      </div>
    </div>

    <!-- context menu -->
    <div v-if="catCtxOpen" class="cat-ctx-overlay" @click="catCtxOpen = false">
      <div class="cat-ctx-menu" @click.stop>
        <button class="cat-ctx-item danger" @click="catCtxDelete">删除分类</button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.cat-modal-body { display: flex; flex-direction: column; gap: 12px; }
.cat-layout { display: flex; gap: 12px; min-height: 360px; }
.cat-sidebar { width: 180px; flex-shrink: 0; display: flex; flex-direction: column; gap: 4px; }
.cat-tab {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 10px; border-radius: var(--radius-xs); cursor: pointer;
  font-size: 12px; color: var(--color-text-secondary); transition: all var(--transition-fast);
}
.cat-tab:hover { background: var(--color-hover); }
.cat-tab.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 500; }
.cat-tab-count { font-size: 10px; color: var(--color-text-tertiary); }
.cat-editor { flex: 1; display: flex; flex-direction: column; gap: 8px; }
.cat-editor-head { display: flex; align-items: center; gap: 10px; }
.cat-editor-count { font-size: 11px; color: var(--color-text-tertiary); }
.cat-editor-area { font-family: 'Cascadia Code', monospace; font-size: 12px; }
.cat-footer { display: flex; align-items: center; gap: 8px; }
.cat-footer-spacer { flex: 1; }
.cat-ctx-overlay { position: fixed; inset: 0; z-index: 9999; }
.cat-ctx-menu {
  position: fixed; background: var(--color-surface); border: 1px solid var(--color-border);
  border-radius: var(--radius-sm); box-shadow: var(--shadow-md); padding: 4px; z-index: 10000;
}
.cat-ctx-item {
  display: block; width: 100%; padding: 6px 16px; border: none; background: none;
  cursor: pointer; font-size: 13px; color: var(--color-text); border-radius: var(--radius-xs);
  text-align: left;
}
.cat-ctx-item:hover { background: var(--color-hover); }
.cat-ctx-item.danger { color: var(--color-error); }
.cat-ctx-item.danger:hover { background: var(--color-error-bg); }
</style>
