<script setup lang="ts">
const bars = Array.from({ length: 7 }, (_, index) => index)
</script>

<template>
  <span class="brand-mark" aria-hidden="true">
    <i v-for="bar in bars" :key="bar"></i>
  </span>
</template>