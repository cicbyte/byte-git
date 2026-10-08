<script setup>
/**
 * A single context-menu entry. Pairs with <ContextMenu>.
 *
 * Style lives globally (see ContextMenu.vue's unscoped block) so the class
 * contract is shared; this component just renders the right markup and wires
 * the click through.
 */
defineProps({
  danger: { type: Boolean, default: false },
})
defineEmits(['click'])
</script>

<template>
  <button
    class="ctx-item"
    :class="{ danger }"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>
