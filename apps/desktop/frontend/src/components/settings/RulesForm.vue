<script setup>
import { ref, onMounted } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import { SaveOutlined, UndoOutlined, GlobalOutlined } from '@ant-design/icons-vue'
import CodeViewer from '../common/CodeViewer.vue'

/**
 * 代理规则管理 — 编辑 ~/.cicbyte/apps/byte-git/rules/rules.lua
 * 定义 get_proxies() + get_accelerated_url()，供 byte-git clone 使用。
 */
const app = window.go?.main?.ByteGitApp

const script = ref('')
const path = ref('')
const loading = ref(false)
const saving = ref(false)
const dirty = ref(false)

async function load() {
  if (!app?.RulesRead) return
  loading.value = true
  try {
    script.value = await app.RulesRead()
    if (app.RulesPath) path.value = await app.RulesPath()
    dirty.value = false
  } catch (e) {
    AMessage.error('加载失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}

function onChange(v) {
  script.value = v
  dirty.value = true
}

async function save() {
  if (!app?.RulesSave) return
  saving.value = true
  try {
    await app.RulesSave(script.value)
    AMessage.success('已保存')
    dirty.value = false
  } catch (e) {
    AMessage.error('保存失败: ' + String(e?.message || e))
  } finally {
    saving.value = false
  }
}

function confirmReset() {
  Modal.confirm({
    title: '恢复默认规则？',
    content: '将用内置模板覆盖当前 rules.lua，你的自定义代理列表会丢失。',
    okText: '恢复', okType: 'danger', cancelText: '取消',
    async onOk() {
      try {
        await app.RulesReset()
        AMessage.success('已恢复默认')
        await load()
      } catch (e) { AMessage.error('恢复失败: ' + String(e?.message || e)) }
    },
  })
}

onMounted(load)
</script>

<template>
  <div class="rf-form">
    <div class="rf-head">
      <div class="rf-title"><GlobalOutlined /> 代理规则</div>
      <span v-if="dirty" class="rf-dirty">未保存 ●</span>
      <span class="rf-path" :title="path">{{ path }}</span>
      <div class="rf-actions">
        <a-button size="small" type="primary" :loading="saving" :disabled="!dirty" @click="save">
          <template #icon><SaveOutlined /></template>保存
        </a-button>
        <a-button size="small" @click="confirmReset" title="恢复默认">
          <template #icon><UndoOutlined /></template>重置
        </a-button>
      </div>
    </div>

    <div class="rf-desc">
      此脚本定义 <code>get_proxies()</code> 返回代理镜像列表，<code>get_accelerated_url(repo_url, proxy_name)</code>
      生成加速 URL。仅被 <code>byte-git clone</code> 使用。
    </div>

    <div class="rf-editor">
      <a-spin v-if="loading" class="rf-spin" />
      <CodeViewer
        v-else
        :code="script"
        filename="rules.lua"
        editable
        @update:modelValue="onChange"
      />
    </div>
  </div>
</template>

<style scoped>
.rf-form { flex: 1; display: flex; flex-direction: column; padding: 20px 24px; background: var(--color-background); overflow: hidden; min-height: 0; }

.rf-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; flex-shrink: 0; }
.rf-title { font-size: 14px; font-weight: 600; color: var(--color-text); display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
.rf-dirty { font-size: 11px; color: var(--color-warning); font-weight: 600; flex-shrink: 0; }
.rf-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-tertiary); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rf-actions { display: flex; gap: 6px; flex-shrink: 0; }

.rf-desc {
  font-size: 12px; color: var(--color-text-tertiary); line-height: 1.6;
  padding: 8px 12px; background: var(--color-surface); border-radius: var(--radius-sm);
  border-left: 3px solid var(--color-primary); margin-bottom: 12px; flex-shrink: 0;
}
.rf-desc code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }

.rf-editor { flex: 1; min-height: 0; position: relative; overflow: hidden; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
.rf-spin { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
</style>
