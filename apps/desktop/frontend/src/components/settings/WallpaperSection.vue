<script setup>
/**
 * WallpaperSection — 壁纸设置 UI（设置 → 外观 → 壁纸子标签）。
 *
 * 三块能力：
 *   1. 总开关 + 上传 + 缩略图网格
 *   2. 三档滑块（模糊度 / 遮罩浓度 / 面板透明度）
 *   3. 用户引导提示
 *
 * 上传流程：WallpaperPickFile（Go 打开原生对话框）→ WallpaperUpload（复制+时间戳）
 * → refreshList → setActive（自动 enabled=true）。
 */
import { ref, onMounted, computed } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  PictureOutlined, UploadOutlined, DeleteOutlined,
  CheckCircleOutlined, InfoCircleOutlined,
} from '@ant-design/icons-vue'
import { useWallpaperStore } from '@/stores/wallpaper'

const wallpaper = useWallpaperStore()

const uploading = ref(false)
const wailsApp = computed(() => window.go?.main?.ByteGitApp ?? null)

/** Lazy per-file thumbnail: fetch data URL on first hover/select. We cache on
 *  the component instance to avoid refetching. */
const thumbCache = ref({})

async function onUpload() {
  if (!wailsApp.value?.WallpaperPickFile) {
    AMessage.error('后端不可用')
    return
  }
  uploading.value = true
  try {
    const src = await wailsApp.value.WallpaperPickFile()
    if (!src) return  // user cancelled
    const filename = await wailsApp.value.WallpaperUpload(src)
    await wallpaper.refreshList()
    wallpaper.setActive(filename)
    AMessage.success('壁纸已设置')
  } catch (e) {
    AMessage.error('上传失败：' + (e?.message || String(e)))
  } finally {
    uploading.value = false
  }
}

async function onPick(name) {
  wallpaper.setActive(name)
}

async function onDelete(name) {
  if (!wailsApp.value?.WallpaperDelete) return
  try {
    // Clear active first if it's the one being deleted.
    if (wallpaper.activeName === name) wallpaper.clear()
    await wailsApp.value.WallpaperDelete(name)
    delete thumbCache.value[name]
    await wallpaper.refreshList()
    AMessage.success('已删除')
  } catch (e) {
    AMessage.error('删除失败：' + (e?.message || String(e)))
  }
}

/** Fetch a single thumbnail's data URL on demand. Uses WallpaperThumbURL
 *  (320px JPEG, cached server-side in .thumbs/) instead of the multi-MB
 *  original — the picker grid loads near-instantly even with many wallpapers. */
async function ensureThumb(name) {
  if (thumbCache.value[name]) return
  if (!wailsApp.value?.WallpaperThumbURL) return
  try {
    thumbCache.value[name] = await wailsApp.value.WallpaperThumbURL(name)
  } catch (e) {
    // ignore — thumbnail just stays blank
  }
}

/* slider value ↔ store value adapters (sliders emit integers) */
const blurValue = computed({
  get: () => wallpaper.blur,
  set: (v) => wallpaper.setBlur(v),
})
const maskPercent = computed({
  get: () => Math.round(wallpaper.mask * 100),
  set: (v) => wallpaper.setMask(v / 100),
})
const panelAlphaPercent = computed({
  get: () => Math.round(wallpaper.panelAlpha * 100),
  set: (v) => wallpaper.setPanelAlpha(v / 100),
})
const navAlphaPercent = computed({
  get: () => Math.round(wallpaper.navAlpha * 100),
  set: (v) => wallpaper.setNavAlpha(v / 100),
})

const sliderDisabled = computed(() => !wallpaper.hasWallpaper || !wallpaper.enabled)

onMounted(async () => {
  // Defensive: if the user opened Settings before ever mounting AppLayout
  // (the normal init site), bootstrap here.
  if (!wallpaper.list.length) {
    await wallpaper.init()
  } else {
    // pre-load thumbnails for visible tiles
    wallpaper.list.slice(0, 12).forEach(ensureThumb)
  }
})
</script>

<template>
  <div class="settings-form">
    <!-- 总开关 -->
    <div class="setting-group">
      <div class="group-title"><PictureOutlined /> 壁纸</div>
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">启用壁纸</div>
          <div class="row-help">开启后，所有面板背景自动半透明透出壁纸。</div>
        </div>
        <a-switch
          :checked="wallpaper.enabled"
          :disabled="!wallpaper.hasWallpaper"
          @change="(v) => wallpaper.setEnabled(v)"
        />
      </div>
    </div>

    <!-- 上传 + 缩略图网格 -->
    <div class="setting-group">
      <div class="group-title"><UploadOutlined /> 壁纸库</div>

      <div class="wp-grid">
        <!-- upload tile -->
        <button
          class="wp-tile wp-tile-upload"
          :disabled="uploading"
          @click="onUpload"
        >
          <UploadOutlined class="upload-icon" />
          <span class="upload-label">{{ uploading ? '上传中…' : '上传壁纸' }}</span>
        </button>

        <!-- empty state -->
        <div v-if="!wallpaper.list.length && !uploading" class="wp-empty">
          暂无壁纸，点击上方按钮上传
        </div>

        <!-- wallpaper tiles -->
        <div
          v-for="name in wallpaper.list"
          :key="name"
          class="wp-tile wp-tile-img"
          :class="{ active: wallpaper.activeName === name }"
          @click="onPick(name)"
          @mouseenter="ensureThumb(name)"
        >
          <div
            class="wp-thumb"
            :style="thumbCache[name] ? { backgroundImage: `url(${thumbCache[name]})` } : {}"
          ></div>
          <button class="wp-del" title="删除" @click.stop="onDelete(name)">
            <DeleteOutlined />
          </button>
          <CheckCircleOutlined v-if="wallpaper.activeName === name" class="wp-active-check" />
          <span class="wp-name">{{ name }}</span>
        </div>
      </div>
    </div>

    <!-- 三档滑块 -->
    <div class="setting-group">
      <div class="group-title">参数</div>

      <div class="slider-row">
        <div class="slider-label">
          <div class="slider-title">壁纸模糊</div>
          <div class="slider-help">壁纸图层自身的模糊程度（不影响面板）。</div>
        </div>
        <div class="slider-control">
          <a-slider v-model:value="blurValue" :min="0" :max="40" :disabled="sliderDisabled" style="width: 140px;" />
          <span class="slider-val">{{ blurValue }}px</span>
        </div>
      </div>

      <div class="slider-row">
        <div class="slider-label">
          <div class="slider-title">遮罩浓度</div>
          <div class="slider-help">压暗/压亮壁纸以保证文字可读（暗色压黑、亮色压白）。</div>
        </div>
        <div class="slider-control">
          <a-slider v-model:value="maskPercent" :min="0" :max="80" :disabled="sliderDisabled" style="width: 140px;" />
          <span class="slider-val">{{ maskPercent }}%</span>
        </div>
      </div>

      <div class="slider-row">
        <div class="slider-label">
          <div class="slider-title">面板透明度</div>
          <div class="slider-help">值越大面板越透明，壁纸透出越明显。</div>
        </div>
        <div class="slider-control">
          <a-slider v-model:value="panelAlphaPercent" :min="0" :max="70" :disabled="sliderDisabled" style="width: 140px;" />
          <span class="slider-val">{{ panelAlphaPercent }}%</span>
        </div>
      </div>

      <div class="slider-row">
        <div class="slider-label">
          <div class="slider-title">导航栏透明度</div>
          <div class="slider-help">项目轨道、菜单栏、顶栏、页脚的透明度。值越大越透出壁纸（上限 60% 保证文字可读）。</div>
        </div>
        <div class="slider-control">
          <a-slider v-model:value="navAlphaPercent" :min="0" :max="60" :disabled="sliderDisabled" style="width: 140px;" />
          <span class="slider-val">{{ navAlphaPercent }}%</span>
        </div>
      </div>
    </div>

    <!-- 用户引导 -->
    <div class="setting-group">
      <div class="group-title"><InfoCircleOutlined /> 说明</div>
      <div class="display-hint">
        <p><CheckCircleOutlined class="hint-icon" /> 推荐比例 16:9，分辨率 ≥ 1920×1080。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 支持 PNG / JPG / WebP / GIF / BMP / AVIF。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 图片存储在 <code>~/.cicbyte/apps/byte-git/wallpapers/</code>，随数据目录备份。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 启用壁纸时建议配合「自定义主题」精细调整面板/文字颜色。</p>
        <p><CheckCircleOutlined class="hint-icon" /> 所有滑块实时预览，关闭壁纸时已上传文件保留。</p>
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
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

/* wallpaper grid */
.wp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 10px;
}
.wp-tile {
  position: relative;
  height: 75px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--color-border);
  overflow: hidden;
  cursor: pointer;
  transition: border-color var(--transition-fast), transform var(--transition-fast);
  background: var(--color-background);
  display: flex; align-items: center; justify-content: center;
  font-family: inherit;
  padding: 0;
}
.wp-tile:hover { transform: translateY(-1px); }
.wp-tile.active {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary);
}
.wp-tile-upload {
  flex-direction: column; gap: 4px;
  border-style: dashed;
  color: var(--color-text-secondary);
}
.wp-tile-upload:hover:not(:disabled) { color: var(--color-primary); border-color: var(--color-primary); }
.wp-tile-upload:disabled { opacity: 0.6; cursor: progress; }
.upload-icon { font-size: 18px; }
.upload-label { font-size: 11px; }

.wp-thumb {
  position: absolute; inset: 0;
  background-size: cover; background-position: center;
}
.wp-del {
  position: absolute; top: 4px; right: 4px;
  width: 20px; height: 20px;
  border: none; border-radius: 50%;
  background: rgba(0,0,0,0.55); color: #fff;
  display: flex; align-items: center; justify-content: center;
  font-size: 11px; cursor: pointer;
  opacity: 0; transition: opacity var(--transition-fast);
}
.wp-tile:hover .wp-del { opacity: 1; }
.wp-del:hover { background: var(--color-error); }
.wp-active-check {
  position: absolute; top: 4px; left: 4px;
  color: var(--color-primary); background: var(--color-surface);
  border-radius: 50%; padding: 1px; font-size: 13px;
}
.wp-name {
  position: absolute; bottom: 0; left: 0; right: 0;
  font-size: 10px; color: #fff;
  background: rgba(0,0,0,0.5);
  padding: 2px 6px;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.wp-empty {
  grid-column: 1 / -1;
  text-align: center; color: var(--color-text-tertiary);
  font-size: 12px; padding: 20px;
}

/* sliders */
.slider-row {
  display: flex; justify-content: space-between; align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--color-border-light);
}
.slider-row:last-child { border-bottom: none; }
.slider-label { display: flex; flex-direction: column; max-width: 60%; }
.slider-title { font-size: 14px; color: var(--color-text); }
.slider-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }
.slider-control { display: flex; align-items: center; gap: 12px; }
.slider-val {
  font-family: 'Cascadia Code', monospace;
  font-size: 12px; color: var(--color-text-secondary);
  min-width: 48px; text-align: right;
}

.display-hint { font-size: 12px; color: var(--color-text-tertiary); line-height: 2; }
.display-hint p { margin: 0; display: flex; align-items: center; gap: 6px; }
.display-hint code {
  font-family: 'Cascadia Code', monospace;
  background: var(--color-background);
  padding: 1px 5px; border-radius: 3px;
  border: 1px solid var(--color-border-light);
  font-size: 11px;
}
.hint-icon { color: var(--color-success); font-size: 13px; }
</style>
