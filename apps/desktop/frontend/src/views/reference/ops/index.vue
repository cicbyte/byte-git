<script setup>
/**
 * 引用管理页 — 当前项目的引用管理操作。
 *
 * 只承载当前项目相关的功能:
 *   - 初始化(reference init)
 *   - 健康检查(reference doctor)
 *   - Wiki 同步(reference wiki sync/commit/remote/trash/restore)
 *
 * 跨项目的全局功能(统计/视图/垃圾回收/网络代理)后续会移到独立的
 * reference-desktop 应用,不在此页堆积。
 *
 * 布局仿 SettingsView:左侧分类菜单,右侧按选中分类切换。
 */
import { ref, onMounted, onBeforeUnmount, h } from 'vue'
import {
  ToolOutlined, HeartOutlined, CloudOutlined, ApartmentOutlined,
  ExclamationCircleOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import { useToolStatus } from '../../../composables/useToolStatus'
import { usePageFooter } from '../../../composables/usePageFooter'
import InitSection from '../../../components/reference/sections/InitSection.vue'
import DoctorSection from '../../../components/reference/sections/DoctorSection.vue'
import WikiSection from '../../../components/reference/sections/WikiSection.vue'
import TrashSection from '../../../components/reference/sections/TrashSection.vue'

const { reference: refStatus, ensureLoaded } = useToolStatus()
const { setFooter, clearFooter } = usePageFooter()

const available = ref(false)

// footer 注意事项(幂等操作等)— 从 InitSection 移到全局 footer,让整页
// 可见,section 主体更干净。
const NoteFooter = () => h('span', { style: 'font-size:12px;color:var(--color-text-tertiary)' }, [
  h('b', { style: 'color:var(--color-text-secondary)' }, '幂等操作'),
  ' · 重复执行安全,已存在的 .reference/ 与配置会保留,仅补全缺失项;重新选择 Agent 会更新注入的 AI 配置文件',
])

const sections = [
  { key: 'init', icon: ToolOutlined, label: '初始化' },
  { key: 'doctor', icon: HeartOutlined, label: '健康检查' },
  { key: 'wiki', icon: CloudOutlined, label: 'Wiki 同步' },
  { key: 'trash', icon: DeleteOutlined, label: '回收站' },
]
const active = ref('init')

onMounted(async () => {
  await ensureLoaded()
  available.value = !!refStatus.value.available
  if (available.value) setFooter(NoteFooter, () => ({}))
})
onBeforeUnmount(() => clearFooter())
</script>

<template>
  <div class="rops-view">
    <!-- not available banner -->
    <div v-if="!available" class="rops-na">
      <ExclamationCircleOutlined class="na-icon" />
      <div class="na-body">
        <div class="na-title">reference CLI 未就绪</div>
        <div class="na-msg">{{ refStatus.message || '请先在设置中配置 reference.exe 路径' }}</div>
      </div>
      <router-link to="/settings" class="na-btn">
        <ToolOutlined /> 去配置
      </router-link>
    </div>

    <!-- main split: left menu + right content -->
    <div v-else class="rops-main">
      <aside class="rops-sider">
        <div class="sider-title">
          <ApartmentOutlined />
          <span>引用管理</span>
        </div>
        <ul class="group-menu">
          <li v-for="s in sections" :key="s.key">
            <a
              class="menu-item"
              :class="{ active: active === s.key }"
              @click="active = s.key"
            >
              <component :is="s.icon" />
              <span>{{ s.label }}</span>
            </a>
          </li>
        </ul>
      </aside>

      <div class="rops-content">
        <InitSection v-if="active === 'init'" />
        <DoctorSection v-else-if="active === 'doctor'" />
        <WikiSection v-else-if="active === 'wiki'" />
        <TrashSection v-else-if="active === 'trash'" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.rops-view {
  /* Negate the parent .content-wrapper's padding (--spacing-lg = 24px) so
     the sider + content fill the whole region: flush with the navbar on top,
     the global Sidebar on the left, and the footer on the bottom — same
     full-bleed treatment as the Settings page. */
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: var(--color-background);
}

/* not available */
.rops-na {
  margin: 24px; padding: 18px 22px;
  display: flex; align-items: center; gap: 14px;
  background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
  border: 1px solid var(--color-warning, #faad14);
  border-radius: var(--radius-md);
}
.na-icon { font-size: 28px; color: var(--color-warning, #faad14); }
.na-body { flex: 1; }
.na-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.na-msg { font-size: 12px; color: var(--color-text-secondary); margin-top: 4px; white-space: pre-wrap; }
.na-btn {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 6px 14px; border-radius: var(--radius-sm);
  background: var(--color-primary); color: #fff; font-size: 12px;
}
.na-btn:hover { opacity: 0.9; color: #fff; }

/* main split */
.rops-main { flex: 1; display: flex; min-height: 0; overflow: hidden; }

/* left sider — mirrors .settings-sider */
.rops-sider {
  display: flex; flex-direction: column;
  width: 180px; flex-shrink: 0;
  border-right: 1px solid var(--color-border);
  /* Match Settings sider padding for visual consistency. */
  padding: var(--spacing-sm) var(--spacing-sm) var(--spacing-sm) var(--spacing-md);
  user-select: none;
  background: var(--color-surface);
}
.sider-title {
  /* 34px fixed header — aligns with Settings sider-title and the column
     headers across Git Log / Time-Travel / Cheat Sheet. */
  display: flex; align-items: center; gap: 8px;
  height: 34px;
  padding: 0 12px;
  margin-bottom: var(--spacing-sm);
  font-size: 14px; font-weight: 600; color: var(--color-text);
}
.sider-title :deep(.anticon) { color: var(--color-primary); }
.group-menu { display: flex; flex-direction: column; list-style: none; margin: 0; padding: 0; }
.group-menu li { margin-bottom: 4px; }
.menu-item {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; width: 100%;
  text-decoration: none; color: var(--color-text); cursor: pointer;
  border-radius: var(--radius-md);
  font-weight: 500; font-size: 13px;
  border: 1px solid transparent;
  transition: all var(--transition-fast);
}
.menu-item :deep(.anticon) { font-size: 14px; opacity: 0.85; }
.menu-item:hover { background: var(--color-hover); color: var(--color-primary); }
.menu-item.active {
  background: var(--color-primary-bg);
  color: var(--color-primary);
  border-color: var(--color-border);
}

/* right content */
.rops-content {
  flex: 1; min-width: 0; min-height: 0;
  display: flex; flex-direction: column;
  overflow: hidden;
}
</style>
