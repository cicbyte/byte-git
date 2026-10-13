<script setup>
/**
 * AnimationForm — 动画偏好设置 UI（设置 → 外观 → 动画子标签）。
 *
 * 6 项偏好绑定到 motion store。所有改动实时通过 gsap.defaults() 生效，
 * 影响所有未显式指定 duration/ease 的 tween。
 */
import { computed } from 'vue'
import {
  ThunderboltOutlined, CheckCircleOutlined,
  MenuOutlined, NodeIndexOutlined, NumberOutlined,
} from '@ant-design/icons-vue'
import { useMotionStore } from '../../stores/motion'

const motion = useMotionStore()

const enabled = computed({
  get: () => motion.prefs.enabled,
  set: (v) => motion.setEnabled(v),
})
const bounce = computed({
  get: () => motion.prefs.bounce,
  set: (v) => motion.setBounce(v),
})
const speed = computed({
  get: () => motion.prefs.speed,
  set: (v) => motion.setSpeed(v),
})
const offset = computed({
  get: () => motion.prefs.offset,
  set: (v) => motion.setOffset(v),
})
const stagger = computed({
  get: () => motion.prefs.stagger,
  set: (v) => motion.setStagger(v),
})
const countUp = computed({
  get: () => motion.prefs.countUp,
  set: (v) => motion.setCountUp(v),
})

// system-level flag — read-only display, always overrides user pref
const systemReduced = computed(() => motion.prefersReduced())
</script>

<template>
  <div class="settings-form">
    <!-- 总开关 -->
    <div class="setting-group">
      <div class="group-title"><ThunderboltOutlined /> 动画</div>
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">启用动画</div>
          <div class="row-help">关闭后所有过渡/入场动画瞬时完成。</div>
        </div>
        <a-switch v-model:checked="enabled" :disabled="systemReduced" />
      </div>
      <div v-if="systemReduced" class="system-notice">
        <CheckCircleOutlined />
        检测到系统已开启「减少动态效果」，动画已被强制禁用（此开关暂不生效）。
      </div>
    </div>

    <!-- 缓动 -->
    <div class="setting-group">
      <div class="group-title">缓动与节奏</div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">回弹效果</div>
          <div class="row-help">开启后入场带轻微 overshoot（back.out），关闭则平滑收尾。</div>
        </div>
        <a-switch v-model:checked="bounce" :disabled="!enabled || systemReduced" />
      </div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">动画速度</div>
          <div class="row-help">整体时长缩放系数，影响所有过渡。</div>
        </div>
        <a-radio-group v-model:value="speed" :disabled="!enabled || systemReduced">
          <a-radio-button value="fast">快</a-radio-button>
          <a-radio-button value="normal">标准</a-radio-button>
          <a-radio-button value="slow">慢</a-radio-button>
        </a-radio-group>
      </div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title"><NodeIndexOutlined /> 入场位移</div>
          <div class="row-help">元素浮现时的位移幅度。</div>
        </div>
        <a-radio-group v-model:value="offset" :disabled="!enabled || systemReduced">
          <a-radio-button value="none">无</a-radio-button>
          <a-radio-button value="light">轻微</a-radio-button>
          <a-radio-button value="strong">明显</a-radio-button>
        </a-radio-group>
      </div>
    </div>

    <!-- 细节 -->
    <div class="setting-group">
      <div class="group-title">细节</div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title"><MenuOutlined /> 列表错峰入场</div>
          <div class="row-help">卡片/列表项依次浮现，而非同时出现。</div>
        </div>
        <a-switch v-model:checked="stagger" :disabled="!enabled || systemReduced" />
      </div>

      <div class="setting-row">
        <div class="row-label">
          <div class="row-title"><NumberOutlined /> 数字滚动</div>
          <div class="row-help">仪表盘统计数字从 0 滚动到目标值。</div>
        </div>
        <a-switch v-model:checked="countUp" :disabled="!enabled || systemReduced" />
      </div>
    </div>

    <!-- 说明 -->
    <div class="setting-group">
      <div class="group-title"><CheckCircleOutlined /> 说明</div>
      <div class="display-hint">
        <p><CheckCircleOutlined class="hint-icon" /> 所有设置即时生效，无需重启。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 速度/回弹通过 gsap.defaults 全局下发，影响所有动画。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 系统「减少动态效果」优先级最高，永远覆盖用户偏好。</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }
.setting-group {
  margin-bottom: var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: 16px 18px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xs);
}
.group-title {
  display: flex; align-items: center; gap: 8px;
  font-size: 14px; font-weight: 600; color: var(--color-text);
  margin-bottom: 12px;
}
.setting-row {
  display: flex; justify-content: space-between; align-items: center;
  min-height: 36px; padding: 6px 0;
}
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); display: flex; align-items: center; gap: 6px; }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

.system-notice {
  display: flex; align-items: center; gap: 6px;
  margin-top: 10px; padding: 8px 10px;
  font-size: 12px; color: var(--color-warning);
  background: var(--color-warning-bg);
  border-radius: var(--radius-sm);
  border-left: 3px solid var(--color-warning);
}

.display-hint { font-size: 12px; color: var(--color-text-tertiary); line-height: 2; }
.display-hint p { margin: 0; display: flex; align-items: center; gap: 6px; }
.hint-icon { color: var(--color-success); font-size: 13px; }
</style>
