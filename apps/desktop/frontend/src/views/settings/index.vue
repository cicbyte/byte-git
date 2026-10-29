<script setup>
import { ref, onMounted } from 'vue'
import { SettingOutlined, GithubOutlined, InfoCircleOutlined, BranchesOutlined, FileSearchOutlined, SafetyCertificateOutlined, CodeOutlined, GlobalOutlined, AppstoreOutlined, ApartmentOutlined, SkinOutlined, BgColorsOutlined, PictureOutlined, ThunderboltOutlined, LayoutOutlined, FileTextOutlined } from '@ant-design/icons-vue'
import AiSparkIcon from '@/components/common/AiSparkIcon.vue'
import SettingsForm from '@/components/settings/SettingsForm.vue'
import PromptForm from '@/components/settings/PromptForm.vue'
import GitConfigForm from '@/components/settings/GitConfigForm.vue'
import OcrForm from '@/components/settings/OcrForm.vue'
import GitleaksForm from '@/components/settings/GitleaksForm.vue'
import ReferenceForm from '@/components/settings/ReferenceForm.vue'
import DisplayForm from '@/components/settings/DisplayForm.vue'
import AppearanceForm from '@/components/settings/AppearanceForm.vue'
import WallpaperSection from '@/components/settings/WallpaperSection.vue'
import AnimationForm from '@/components/settings/AnimationForm.vue'
import HookForm from '@/components/settings/HookForm.vue'
import RulesForm from '@/components/settings/RulesForm.vue'
import TerminalForm from '@/components/settings/TerminalForm.vue'
import FeatureMatrix from '@/components/settings/FeatureMatrix.vue'
import GhForm from '@/components/settings/GhForm.vue'
import { useSettings } from '@/composables/useSettings'

const { load } = useSettings()

const tabs = [
  { key: 'ai', icon: AiSparkIcon, label: 'AI 设置' },
  { key: 'prompts', icon: FileTextOutlined, label: '提示词' },
  { key: 'appearance', icon: SkinOutlined, label: '外观', hasSub: true },
  { key: 'gitconfig', icon: BranchesOutlined, label: 'Git 配置' },
  { key: 'ocr', icon: FileSearchOutlined, label: '代码审查' },
  { key: 'gitleaks', icon: SafetyCertificateOutlined, label: '密钥扫描' },
  { key: 'reference', icon: ApartmentOutlined, label: '引用仓库' },
  { key: 'hook', icon: CodeOutlined, label: 'Hook' },
  { key: 'github', icon: GithubOutlined, label: 'GitHub' },
  { key: 'rules', icon: GlobalOutlined, label: '代理规则' },
  { key: 'terminal', icon: CodeOutlined, label: '终端' },
  { key: 'features', icon: AppstoreOutlined, label: '功能特性' },
  { key: 'about', icon: InfoCircleOutlined, label: '关于' },
]
const activeTab = ref('ai')

// Sub-tabs under the "外观" group: theme / wallpaper / animation / layout.
const appearanceSubs = [
  { key: 'theme', icon: BgColorsOutlined, label: '主题' },
  { key: 'wallpaper', icon: PictureOutlined, label: '壁纸' },
  { key: 'animation', icon: ThunderboltOutlined, label: '动画' },
  { key: 'layout', icon: LayoutOutlined, label: '布局' },
]
const appearanceSub = ref('theme')

onMounted(() => {
  load()
})
</script>

<template>
  <div class="settings-view">
    <!-- Column 1: group menu -->
    <aside class="settings-sider">
      <div class="sider-title">
        <SettingOutlined />
        <span>设置</span>
      </div>
      <ul class="group-menu">
        <li v-for="t in tabs" :key="t.key">
          <a
            class="menu-item"
            :class="{ active: activeTab === t.key }"
            @click="activeTab = t.key"
          >
            <component :is="t.icon" />
            <span>{{ t.label }}</span>
          </a>
        </li>
      </ul>
    </aside>

    <!-- Column 2: sub-menu (only for groups that have sub-categories, e.g. 外观) -->
    <aside v-if="activeTab === 'appearance'" class="settings-subsider">
      <div class="subsider-title">
        <SkinOutlined />
        <span>外观</span>
      </div>
      <ul class="group-menu">
        <li v-for="s in appearanceSubs" :key="s.key">
          <a
            class="menu-item"
            :class="{ active: appearanceSub === s.key }"
            @click="appearanceSub = s.key"
          >
            <component :is="s.icon" />
            <span>{{ s.label }}</span>
          </a>
        </li>
      </ul>
    </aside>

    <!-- Column 3: form content -->
    <GitConfigForm v-if="activeTab === 'gitconfig'" class="settings-content" />
    <PromptForm v-else-if="activeTab === 'prompts'" class="settings-content" />
    <template v-else-if="activeTab === 'appearance'">
      <AppearanceForm v-if="appearanceSub === 'theme'" class="settings-content" />
      <WallpaperSection v-else-if="appearanceSub === 'wallpaper'" class="settings-content" />
      <AnimationForm v-else-if="appearanceSub === 'animation'" class="settings-content" />
      <DisplayForm v-else class="settings-content" />
    </template>
    <OcrForm v-else-if="activeTab === 'ocr'" class="settings-content" />
    <GitleaksForm v-else-if="activeTab === 'gitleaks'" class="settings-content" />
    <ReferenceForm v-else-if="activeTab === 'reference'" class="settings-content" />
    <HookForm v-else-if="activeTab === 'hook'" class="settings-content" />
    <RulesForm v-else-if="activeTab === 'rules'" class="settings-content" />
    <GhForm v-else-if="activeTab === 'github'" class="settings-content" />
    <TerminalForm v-else-if="activeTab === 'terminal'" class="settings-content" />
    <div v-else-if="activeTab === 'features'" class="settings-content features-page">
      <FeatureMatrix />
    </div>
    <SettingsForm v-else :active-tab="activeTab" class="settings-content" />
  </div>
</template>

<style scoped>
.settings-view {
  display: flex;
  /* Negate the parent .content-wrapper's padding (--spacing-lg = 24px) so
     the sider + content fill the whole region: flush with the navbar on
     top, the global Sidebar on the left, and the footer on the bottom. */
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  overflow: hidden;
  background: var(--bg-content);
}

/* Right content area: fill remaining space after sidebar */
.settings-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* features page: scrollable with padding */
.features-page {
  overflow-y: auto;
  padding: 20px 24px;
  background: var(--bg-content);
}

/* Left group menu */
.settings-sider {
  display: flex;
  flex-direction: column;
  min-width: 200px;
  width: 200px;
  border-right: 1px solid var(--color-border);
  /* Comfortable inner padding on all sides; slightly more on top to separate
     the title from the navbar. */
  padding: var(--spacing-sm) var(--spacing-sm) var(--spacing-sm) var(--spacing-md);
  user-select: none;
  flex-shrink: 0;
  /* Use the base surface color (not the sider region var) — the settings
     sider is a sub-panel INSIDE the content area, not the global sidebar.
     Keeping it on --bg-surface decouples it from the "菜单栏" region override
     so editing 菜单栏 background no longer affects the settings tab strip. */
  background: var(--bg-surface);
}

.sider-title {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px 14px;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text);
}

.group-menu {
  display: flex;
  flex-direction: column;
  list-style: none;
  margin: 0;
  padding: 0;
}

.group-menu li {
  margin-bottom: 4px;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  width: 100%;
  text-decoration: none;
  color: var(--color-text);
  cursor: pointer;
  border-radius: var(--radius-md);
  font-weight: 500;
  font-size: 14px;
  border: 1px solid transparent;
  transition: all var(--transition-fast);
}

.menu-item .anticon {
  font-size: 15px;
  opacity: 0.85;
}

.menu-item:hover {
  background: var(--color-hover);
  color: var(--color-primary);
}

.menu-item.active {
  background: var(--color-primary-bg);
  color: var(--color-primary);
  border-color: var(--color-border);
}

/* Column 2 — sub-sider (e.g. 外观 → 主题/壁纸/动画/布局).
   Mirrors .settings-sider styling but slightly narrower. */
.settings-subsider {
  display: flex;
  flex-direction: column;
  min-width: 160px;
  width: 160px;
  border-right: 1px solid var(--color-border);
  /* Match .settings-sider padding for visual consistency. */
  padding: var(--spacing-sm) var(--spacing-sm) var(--spacing-sm) var(--spacing-md);
  user-select: none;
  flex-shrink: 0;
  background: var(--bg-surface);
}
.subsider-title {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px 14px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text);
}
.subsider-title .anticon { color: var(--color-primary); font-size: 14px; }

@media (max-width: 768px) {
  .settings-sider {
    min-width: 64px;
    width: 64px;
  }
  .settings-subsider {
    min-width: 56px;
    width: 56px;
  }
  .sider-title span,
  .subsider-title span,
  .menu-item span {
    display: none;
  }
  .menu-item {
    justify-content: center;
  }
}
</style>
