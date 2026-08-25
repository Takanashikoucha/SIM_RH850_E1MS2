<script setup lang="ts">
defineProps<{
  lastPc: number;
  cause: number;
}>();
</script>

<template>
  <div class="fault-banner">
    <span class="icon">⚠</span>
    <span>
      PC fell off loaded code at <b>0x{{ lastPc.toString(16).padStart(8, "0") }}</b>
      — cause <b>0x{{ cause.toString(16) }}</b>
      ({{ cause === 0x60 ? "RIE (Return from Exception)" : cause === 0x31 ? "FETRAP" : cause === 0x10 ? "SYSERR" : "exception" }})
    </span>
  </div>
</template>
