<script setup lang="ts">
import { computed } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();

const pcHex = computed(() => sim.pc.value.toString(16).padStart(8, "0"));
const insCount = computed(() => sim.insCount.value);
const pswHex = computed(() => sim.psw.value.toString(16).padStart(8, "0"));
const rbase = computed(() => sim.rbaseInput.value);
const romLoaded = computed(() => sim.romLoaded.value);
const romLen = computed(() => sim.romLen.value);
const bpCount = computed(() => sim.breakpoints.value.size);
const watchCount = computed(() => sim.watch.value.length);
const stopReason = computed(() => sim.stopReason.value);
</script>

<template>
  <div class="status-bar">
    <div class="status-item">
      <span class="label">PC</span>
      <span class="value highlight">0x{{ pcHex }}</span>
    </div>
    <div class="status-item">
      <span class="label">INS</span>
      <span class="value">{{ insCount }}</span>
    </div>
    <div class="status-item">
      <span class="label">PSW</span>
      <span class="value">0x{{ pswHex }}</span>
    </div>
    <div class="status-item">
      <span class="label">RBASE</span>
      <span class="value">0x{{ rbase }}</span>
    </div>
    <div class="status-item">
      <span class="label">ROM</span>
      <span class="value" :style="{ color: romLoaded ? 'var(--green)' : 'var(--red)' }">
        {{ romLoaded ? `${romLen} bytes` : "not loaded" }}
      </span>
    </div>
    <div class="status-item">
      <span class="label">BP</span>
      <span class="value">{{ bpCount }}</span>
    </div>
    <div class="status-item">
      <span class="label">WATCH</span>
      <span class="value">{{ watchCount }}</span>
    </div>
    <div class="status-item" v-if="stopReason">
      <span class="label">STOP</span>
      <span class="value" style="color: var(--yellow)">{{ stopReason }}</span>
    </div>
  </div>
</template>
