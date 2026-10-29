<script setup>
/**
 * Right-click context menu for the .gitignore view.
 *
 * Renders the <a-menu> used as the overlay of the contextmenu dropdown.
 * 所有条目均为单层（a-menu-item-group 分组标题不弹出）——dropdown 内
 * 嵌套 submenu 的弹层在重开后会残留幽灵层（表现为两个菜单叠加），
 * 因此「添加规则」用分组标题承载，本地模板走弹窗选择。
 * All data is passed as props and all actions are emitted to the parent
 * (index.vue owns the state and the handlers).
 */
import {
  DatabaseOutlined, GithubOutlined, SettingOutlined,
  ThunderboltOutlined, SearchOutlined, ReloadOutlined,
} from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'

defineProps({
  localTemplates: { type: Array, default: () => [] },
  githubCloned: { type: Boolean, default: false },
  githubTemplateCount: { type: Number, default: 0 },
})

defineEmits([
  'openLocalPicker',
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
    <!-- 添加规则（分组标题，不弹出） -->
    <a-menu-item-group title="添加规则">
      <a-menu-item key="add-local" @click="$emit('openLocalPicker')">
        <template #icon><DatabaseOutlined /></template>
        本地模板{{ localTemplates.length ? ` (${localTemplates.length})` : '…' }}
      </a-menu-item>
      <a-menu-item key="add-github" @click="$emit('openGithubPicker')">
        <template #icon><GithubOutlined /></template>
        GitHub 模板{{ githubCloned ? ` (${githubTemplateCount})` : '…' }}
      </a-menu-item>
      <a-menu-item key="add-ai" @click="$emit('showAiInput')">
        <template #icon><AiSparkIcon /></template>
        AI 生成…
      </a-menu-item>
    </a-menu-item-group>

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
