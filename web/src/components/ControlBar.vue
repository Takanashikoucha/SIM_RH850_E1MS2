<script setup lang="ts">
import { computed } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();

const isRunning = computed(() => sim.running.value);
const isStopped = computed(() => sim.stopped.value);
const insCount = computed(() => sim.insCount.value);
</script>

<template>
  <div class="controls">
    <button class="btn" @click="sim.doReset()" title="Reset (R)">
      ↺ <span class="kbd">R</span>
    </button>
    <button class="btn primary" @click="sim.doRun()" :disabled="isRunning" title="Run (F5)">
      ▶ Run <span class="kbd">F5</span>
    </button>
    <button class="btn" @click="sim.doStep()" :disabled="isRunning || isStopped" title="Step (F10)">
      → Step <span class="kbd">F10</span>
    </button>
    <button class="btn" @click="sim.doStepOver()" :disabled="isRunning || isStopped" title="Step Over (F11)">
      ⤼ Over <span class="kbd">F11</span>
    </button>
    <button class="btn danger" @click="sim.doStop()" :disabled="!isRunning" title="Stop (Esc)">
      ■ Stop <span class="kbd">Esc</span>
    </button>
    <div style="width: 1px; height: 20px; background: var(--border); margin: 0 4px;" />
    <button class="btn" @click="sim.clearBps()" title="Clear all breakpoints">
      ✕ BPs
    </button>
    <div class="spacer" style="flex: 1" />
    <span style="font-size: 11px; color: var(--text-dim); font-family: var(--font-mono);">
      INS {{ insCount }}
    </span>
  </div>
</template>
