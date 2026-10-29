<script setup>
/**
 * 模板管理页 — .gitignore 与 LICENSE 模板的查看、编辑、维护。
 *
 * 独立整页双栏布局（替代原 ignore 页内的管理弹窗）：
 * 左列表（搜索/来源/新建）+ 右编辑器。tab 由路由 query 控制
 * （/templates?tab=license），供 ignore / license 页入口直达。
 */
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import GitignorePane from './GitignorePane.vue'
import LicensePane from './LicensePane.vue'

const route = useRoute()
const tab = ref(route.query.tab === 'license' ? 'license' : 'gitignore')
watch(() => route.query.tab, (v) => {
  if (v === 'license' || v === 'gitignore') tab.value = v
})
</script>

<template>
  <div class="tp-page">
    <div class="tp-head">
      <h2>模板管理</h2>
      <span class="tp-desc">.gitignore 与 LICENSE 模板的查看、编辑与维护</span>
    </div>
    <div class="tp-body">
      <a-tabs v-model:activeKey="tab" class="tp-tabs">
        <a-tab-pane key="gitignore" tab=".gitignore 模板">
          <GitignorePane v-if="tab === 'gitignore'" />
        </a-tab-pane>
        <a-tab-pane key="license" tab="LICENSE 模板">
          <LicensePane v-if="tab === 'license'" />
        </a-tab-pane>
      </a-tabs>
    </div>
  </div>
</template>

<style scoped>
.tp-page {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.tp-head {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin-bottom: var(--spacing-md);
}

.tp-head h2 {
  font-size: 20px;
  font-weight: 600;
  color: var(--color-text);
  margin: 0;
}

.tp-desc {
  font-size: 13px;
  color: var(--color-text-tertiary);
}

.tp-body {
  flex: 1;
  min-height: 0;
}

.tp-tabs {
  height: 100%;
}

.tp-tabs :deep(.ant-tabs-nav) {
  margin-bottom: var(--spacing-md);
}

.tp-tabs :deep(.ant-tabs-content-holder),
.tp-tabs :deep(.ant-tabs-content),
.tp-tabs :deep(.ant-tabs-tabpane) {
  height: 100%;
  min-height: 0;
}
</style>
