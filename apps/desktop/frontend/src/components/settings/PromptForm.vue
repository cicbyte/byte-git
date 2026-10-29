<script setup>
/**
 * 提示词设置页（独立页签）：AI 生成/改写提交信息的系统提示词编辑。
 *
 * - 场景以页内 tab 切换（提交信息生成 aicommit.md / 提交信息改写 airewrite.md）
 * - 各场景内容独立缓存，切 tab 不丢失未保存编辑；保存/恢复默认作用于当前场景
 * - 保存即时落盘 ~/.cicbyte/apps/byte-git/prompts/，下一次生成即生效
 */
import { ref, reactive, computed, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { SaveOutlined, UndoOutlined } from '@ant-design/icons-vue'
import { useSettings, SETTING_KEYS } from '@/composables/useSettings'

const app = window.go?.main?.ByteGitApp
const { settings, queueSave } = useSettings()

const langOptions = [
  { label: '中文', value: 'zh' },
  { label: 'English', value: 'en' },
]
const onLangChange = (v) => queueSave(SETTING_KEYS.lang, v)
// 提示词中的语言占位符（拼接避免 Vue 模板插值解析）
const LANG_PH = '{{' + 'language' + '}}'

const SCENES = [
  { key: 'aicommit.md', label: '提交信息生成', help: '提交台「AI 生成」：根据暂存区 diff 生成 commit message' },
  { key: 'airewrite.md', label: '提交信息改写', help: 'AI 改写：把原始 commit message 整理为标准格式（附 diff 核对）' },
]

const activeScene = ref('aicommit.md')
const contents = reactive({}) // 场景名 -> 编辑中内容（缓存，切 tab 不丢）
const loaded = reactive({}) // 场景名 -> 是否已从后端加载
const loading = ref(false)
const saving = ref(false)

const sceneMeta = computed(() => SCENES.find((s) => s.key === activeScene.value))
const content = computed({
  get: () => contents[activeScene.value] || '',
  set: (v) => {
    contents[activeScene.value] = v
  },
})

async function ensureLoaded(name) {
  if (loaded[name] || !app?.AiPromptGet) return
  loading.value = true
  try {
    const r = await app.AiPromptGet(name)
    contents[name] = r?.content || ''
    loaded[name] = true
  } catch (e) {
    AMessage.error('读取提示词失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}
watch(activeScene, (n) => ensureLoaded(n), { immediate: true })

async function save() {
  if (!app?.AiPromptSave) return
  saving.value = true
  try {
    await app.AiPromptSave(activeScene.value, contents[activeScene.value] || '')
    AMessage.success('已保存，下次生成即生效')
  } catch (e) {
    AMessage.error(String(e?.message || e))
  } finally {
    saving.value = false
  }
}

async function resetDefault() {
  if (!app?.AiPromptReset) return
  try {
    const r = await app.AiPromptReset(activeScene.value)
    contents[activeScene.value] = r?.content || ''
    loaded[activeScene.value] = true
    AMessage.success('已恢复内置默认')
  } catch (e) {
    AMessage.error(String(e?.message || e))
  }
}
</script>

<template>
  <div class="settings-form prompt-form">
    <a-tabs v-model:activeKey="activeScene" size="small">
      <a-tab-pane v-for="s in SCENES" :key="s.key" :tab="s.label" />
    </a-tabs>

    <div class="scene-bar">
      <span class="scene-help">{{ sceneMeta?.help }}</span>
      <span class="lang-pick">
        生成语言
        <a-select
          :value="settings.lang"
          :options="langOptions"
          size="small"
          style="width: 96px"
          @change="onLangChange"
        />
        <span class="lang-note">填充提示词中的 {{ LANG_PH }} 占位符（与 AI 设置同步）</span>
      </span>
    </div>

    <a-spin :spinning="loading" wrapper-class-name="prompt-spin">
      <a-textarea v-model:value="content" :rows="18" class="prompt-area" spellcheck="false" />
    </a-spin>

    <div class="prompt-actions">
      <a-button type="primary" :loading="saving" @click="save">
        <SaveOutlined /> 保存
      </a-button>
      <a-button @click="resetDefault">
        <UndoOutlined /> 恢复默认
      </a-button>
      <span class="prompt-note">保存后下一次生成/改写即生效；模板占位符（language / original / files / diff 等）由应用在请求时填充</span>
    </div>
  </div>
</template>

<style scoped>
.prompt-form {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;
  background: var(--color-background);
  display: flex;
  flex-direction: column;
}

.scene-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.scene-help {
  font-size: 12px;
  color: var(--color-text-tertiary);
}

.lang-pick {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.lang-note {
  font-size: 11px;
  color: var(--color-text-tertiary);
}

.prompt-spin,
.prompt-spin :deep(.ant-spin-container) {
  display: flex;
  flex: 1;
  min-height: 0;
}

.prompt-area {
  flex: 1;
  min-height: 320px;
  font-family: 'Cascadia Code', 'Fira Code', monospace;
  font-size: 12px;
  line-height: 1.6;
  resize: vertical;
}

.prompt-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}

.prompt-note {
  font-size: 11px;
  color: var(--color-text-tertiary);
  margin-left: 8px;
}
</style>
