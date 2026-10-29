<script setup>
/**
 * Right-click context menu for the .gitignore view.
 *
 * Renders the <a-menu> used as the overlay of the contextmenu dropdown.
 * All data is passed as props and all actions are emitted to the parent
 * (index.vue owns the state and the handlers).
 */
import {
  PlusOutlined, DatabaseOutlined, GithubOutlined, SettingOutlined,
  ThunderboltOutlined, SearchOutlined, ReloadOutlined,
} from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'

defineProps({
  localTemplates: { type: Array, default: () => [] },
  githubCloned: { type: Boolean, default: false },
  githubTemplateCount: { type: Number, default: 0 },
})

defineEmits([
  'addFromTemplate', // (name, source)
  'openGithubPicker',
  'showAiInput',
  'openMgr',
  'doRefactor',
  'openCategories',
  'openCheck',
  'loadGitignore',
])
</script>

<template>
  <a-menu>
    <!-- 添加规则 (submenu) -->
    <a-sub-menu key="add" title="添加规则">
      <template #icon><PlusOutlined /></template>
      <!-- local templates -->
      <a-sub-menu key="add-local" title="本地模板">
        <template #icon><DatabaseOutlined /></template>
        <a-menu-item v-for="t in localTemplates" :key="'l-'+t" @click="$emit('addFromTemplate', t, 'local')">
          {{ t }}
        </a-menu-item>
        <a-menu-item v-if="!localTemplates.length" disabled>无模板</a-menu-item>
      </a-sub-menu>
      <!-- github templates -->
      <a-menu-item key="add-github" @click="$emit('openGithubPicker')">
        <template #icon><GithubOutlined /></template>
        GitHub 模板{{ githubCloned ? ` (${githubTemplateCount})` : '…' }}
      </a-menu-item>
      <!-- AI -->
      <a-menu-item key="add-ai" @click="$emit('showAiInput')">
        <template #icon><AiSparkIcon /></template>
        AI 生成…
      </a-menu-item>
    </a-sub-menu>

    <a-menu-divider />

    <!-- 管理模板 -->
    <a-menu-item key="mgr" @click="$emit('openMgr')">
      <template #icon><SettingOutlined /></template>
      管理模板
    </a-menu-item>

    <!-- 整理规则 -->
    <a-menu-item key="refactor" @click="$emit('doRefactor')">
      <template #icon><ThunderboltOutlined /></template>
      整理规则
    </a-menu-item>

    <!-- 分类规则 -->
    <a-menu-item key="categories" @click="$emit('openCategories')">
      <template #icon><SettingOutlined /></template>
      分类规则
    </a-menu-item>

    <!-- 检查文件 -->
    <a-menu-item key="check" @click="$emit('openCheck')">
      <template #icon><SearchOutlined /></template>
      检查文件
    </a-menu-item>

    <a-menu-divider />

    <!-- 刷新 -->
    <a-menu-item key="reload" @click="$emit('loadGitignore')">
      <template #icon><ReloadOutlined /></template>
      刷新
    </a-menu-item>
  </a-menu>
</template>
