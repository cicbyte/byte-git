<script setup>
/**
 * 规则管理弹窗 — 从 AIRewriteView 拆出。
 * 维护检测正则规则 + AI 生成规则。
 */
import { ref, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { ThunderboltOutlined, DeleteOutlined } from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const rules = ref([])
const newRule = ref({ name: '', pattern: '', desc: '' })
const aiDesc = ref('')
const aiGenerating = ref(false)

async function loadRules() {
  if (!app?.RewriteRulesLoad) return
  try { rules.value = await app.RewriteRulesLoad() } catch {}
}

function addRule() {
  if (!newRule.value.name.trim() || !newRule.value.pattern.trim()) {
    AMessage.warning('规则名称和正则不能为空')
    return
  }
  rules.value.push({ ...newRule.value })
  newRule.value = { name: '', pattern: '', desc: '' }
}

async function aiGenerateRule() {
  if (!aiDesc.value.trim()) { AMessage.warning('请描述要检测的提交特征'); return }
  aiGenerating.value = true
  try {
    const rule = await app.RewriteRuleGenerate(aiDesc.value)
    rules.value.push({ name: rule.name || 'AI规则', pattern: rule.pattern, desc: rule.desc || aiDesc.value })
    aiDesc.value = ''
    AMessage.success(`AI 已生成规则: ${rule.name}`)
  } catch (e) {
    AMessage.error('AI 生成失败: ' + String(e?.message || e))
  } finally {
    aiGenerating.value = false
  }
}

function removeRule(idx) { rules.value.splice(idx, 1) }

async function saveRules() {
  try {
    await app.RewriteRulesSave(rules.value)
    AMessage.success(`已保存 ${rules.value.length} 条规则`)
    emit('update:open', false)
  } catch (e) {
    AMessage.error('保存失败: ' + String(e?.message || e))
  }
}

async function resetRules() {
  try {
    rules.value = await app.RewriteRulesReset()
    AMessage.success('已重置为默认规则')
  } catch (e) {
    AMessage.error('重置失败: ' + String(e?.message || e))
  }
}

watch(() => props.open, (v) => { if (v) loadRules() })
</script>

<template>
  <a-modal :open="open" @update:open="(v) => emit('update:open', v)"
    title="检测规则管理" width="680px" :footer="null" destroy-on-close>
    <div class="rules-modal">
      <div class="rules-hint">维护用于识别"不规范" commit message 的正则规则。「按规则匹配」按钮按这些规则过滤。</div>
      <div class="rules-list">
        <div class="rules-row rules-head">
          <span class="rl-name">名称</span><span class="rl-pattern">正则</span><span class="rl-desc">说明</span><span class="rl-op"></span>
        </div>
        <div v-for="(r, idx) in rules" :key="idx" class="rules-row">
          <input class="rl-input rl-name" v-model="r.name" />
          <input class="rl-input rl-pattern" v-model="r.pattern" />
          <input class="rl-input rl-desc" v-model="r.desc" placeholder="可选" />
          <button class="rl-del" @click="removeRule(idx)" title="删除"><DeleteOutlined /></button>
        </div>
      </div>

      <div class="rules-ai">
        <ThunderboltOutlined class="rules-ai-icon" />
        <input class="rl-input rules-ai-input" v-model="aiDesc" placeholder="描述要检测的特征，如「以动词过去式开头」「含 emoji」" @keydown.enter="aiGenerateRule" />
        <a-button size="small" type="primary" :loading="aiGenerating" @click="aiGenerateRule">
          <template #icon><ThunderboltOutlined /></template>AI 生成
        </a-button>
      </div>

      <div class="rules-add">
        <input class="rl-input rl-name" v-model="newRule.name" placeholder="名称" />
        <input class="rl-input rl-pattern" v-model="newRule.pattern" placeholder="正则" />
        <input class="rl-input rl-desc" v-model="newRule.desc" placeholder="说明" />
        <a-button size="small" type="dashed" @click="addRule"><DeleteOutlined />添加</a-button>
      </div>
      <div class="rules-actions">
        <a-button size="small" @click="resetRules">重置默认</a-button>
        <span class="rules-spacer"></span>
        <a-button size="small" @click="emit('update:open', false)">取消</a-button>
        <a-button size="small" type="primary" @click="saveRules">保存</a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.rules-modal { display: flex; flex-direction: column; gap: 12px; }
.rules-hint { font-size: 12px; color: var(--color-text-tertiary); line-height: 1.6; }
.rules-list { display: flex; flex-direction: column; gap: 4px; }
.rules-row { display: grid; grid-template-columns: 100px 1fr 120px 32px; gap: 8px; align-items: center; }
.rules-head { font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); padding-bottom: 4px; border-bottom: 1px solid var(--color-border-light); }
.rl-input { width: 100%; padding: 4px 8px; font-size: 12px; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-background); color: var(--color-text); outline: none; font-family: 'Cascadia Code', monospace; }
.rl-input:focus { border-color: var(--color-primary); }
.rl-del { border: none; background: transparent; cursor: pointer; color: var(--color-text-tertiary); padding: 4px; border-radius: 4px; transition: all 0.15s; }
.rl-del:hover { color: var(--color-error); background: var(--color-hover); }
.rules-ai { display: flex; align-items: center; gap: 8px; padding: 8px 10px; background: var(--color-primary-bg); border: 1px dashed var(--color-primary); border-radius: var(--radius-sm); }
.rules-ai-icon { color: var(--color-primary); flex-shrink: 0; }
.rules-ai-input { flex: 1; }
.rules-add { display: grid; grid-template-columns: 100px 1fr 120px auto; gap: 8px; align-items: center; padding-top: 8px; border-top: 1px solid var(--color-border-light); }
.rules-actions { display: flex; align-items: center; gap: 8px; padding-top: 4px; }
.rules-spacer { flex: 1; }
</style>
