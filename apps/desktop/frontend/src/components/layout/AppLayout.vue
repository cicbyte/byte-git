<script setup>
import { onMounted } from 'vue'
import { useLayoutStore } from '../../stores/layout'
import { useWallpaperStore } from '../../stores/wallpaper'
import ProjectRail from './ProjectRail.vue'
import Sidebar from './Sidebar.vue'
import Navbar from './Navbar.vue'
import MainContent from './MainContent.vue'

const layout = useLayoutStore()
const wallpaper = useWallpaperStore()

// Boot wallpaper state (load prefs → fetch list → validate → apply CSS vars).
// Runs on the main layout mount; if the user opens the Settings → Wallpaper
// tab first (before visiting the main layout), WallpaperSection re-runs init.
onMounted(() => {
  wallpaper.init()
})
</script>

<template>
  <div class="app-layout">
    <!-- Wallpaper layer (bottom, z:0). Own filter does the blur; the
         pointer-events:none keeps it decorative only. -->
    <div
      v-if="wallpaper.enabled && wallpaper.hasWallpaper"
      class="wp-layer"
      :style="{ backgroundImage: `url(${wallpaper.url})`,
                filter: `blur(${wallpaper.blur}px) saturate(1.1)` }"
    ></div>
    <!-- Mask layer (z:1) — color wash that tracks light/dark to keep
         text readable. Black wash in dark, white wash in light. -->
    <div
      v-if="wallpaper.enabled && wallpaper.hasWallpaper"
      class="wp-mask"
      :style="{ background: wallpaper.maskColor }"
    ></div>

    <ProjectRail v-show="layout.projectRailVisible" />
    <Sidebar />
    <div class="app-right">
      <Navbar />
      <MainContent />
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  position: relative;
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: var(--bg-base);  /* opaque when wallpaper off; translucent when on */
}

/* Two wallpaper layers — both absolute, inset:0, pointer-events:none so
   they sit purely behind the layout. */
.wp-layer,
.wp-mask {
  position: absolute;
  inset: 0;
  pointer-events: none;
}
.wp-layer {
  z-index: 0;
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
}
.wp-mask {
  z-index: 1;
}

.app-right {
  position: relative;
  z-index: 2;  /* keep panel content above wallpaper + mask */
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  /* allow the column to shrink so the footer can pin to the bottom */
  min-height: 0;
}
</style>
