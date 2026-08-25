<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();
const baseInput = ref("00000000");
const base = ref(0x00000000);
// "follow" mode: memory panel tracks PC (shows the current instruction region).
// Switches to fixed mode once the user clicks an address or uses the Go input.
const follow = ref(true);
const rowsToShow = 16; // 16 rows × 8 bytes = 128 bytes

function alignDown(v: number, align: number): number {
  return (v - (v % align)) >>> 0;
}

function applyBase() {
  const v = parseInt(baseInput.value, 16) >>> 0;
  base.value = v;
  follow.value = false;
}

const viewBase = computed(() => {
  if (!follow.value) return base.value;
  // Follow PC: 16-byte aligned window containing PC
  const pc = sim.pc.value;
  return alignDown(pc, 16);
});

const rows = computed(() => {
  // Depend on memVersion so the computed re-reads after loadRom/reset.
  void sim.memVersion.value;
  const s = sim.sim.value;
  if (!s) return [];
  const out: { addr: number; bytes: number[]; words: number[]; isPcRow: boolean }[] = [];
  const start = viewBase.value;
  const pc = sim.pc.value;
  for (let i = 0; i < rowsToShow; i++) {
    const addr = (start + i * 8) >>> 0;
    const bytes: number[] = [];
    const words: number[] = [];
    for (let j = 0; j < 8; j++) {
      bytes.push(s.getMemByte((addr + j) >>> 0));
    }
    // Compute words for better readability
    for (let j = 0; j < 8; j += 4) {
      words.push(s.getMemWord((addr + j) >>> 0));
    }
    // Mark the row containing PC
    const isPcRow = follow.value && pc >= addr && pc < addr + 8;
    out.push({ addr, bytes, words, isPcRow });
  }
  return out;
});

function onAddrClick(addr: number) {
  base.value = addr >>> 0;
  baseInput.value = addr.toString(16).padStart(8, "0");
  follow.value = false;
}

// When following, keep the address input in sync so the user sees the live base.
watch(viewBase, (v) => {
  if (follow.value) {
    baseInput.value = v.toString(16).padStart(8, "0");
  }
});

function regionName(addr: number): string {
  if (addr < 0x400000) return "FLASH";
  if (addr >= 0xFEBF0000 && addr < 0xFEC00000) return "L-RAM";
  if (addr >= 0xFEDF0000 && addr < 0xFEE00000) return "L-RAM2";
  if (addr >= 0xFEEE0000 && addr < 0xFEF00000) return "G-RAM";
  if ((addr >= 0x10000000 && addr < 0x20000000) || (addr >= 0xFF000000)) return "I/O";
  return "";
}

const currentRegion = computed(() => regionName(viewBase.value));
</script>

<template>
  <div class="panel" style="flex: 1;">
    <div class="panel-title">
      Memory
      <span v-if="follow" style="margin-left: 4px; font-size: 0.75em; color: var(--accent);" title="Memory follows PC; click an address or use Go to pin a fixed window">
        [follow PC]
      </span>
      <span v-else-if="currentRegion" style="margin-left: 4px; font-size: 0.75em; color: var(--green);">
        [{{ currentRegion }}]
      </span>
      <div style="margin-left: auto; display: flex; gap: 4px; align-items: center;">
        <input
          class="input"
          style="width: 80px;"
          v-model="baseInput"
          @keyup.enter="applyBase()"
        />
        <button class="btn" style="padding: 2px 6px;" @click="applyBase()">Go</button>
        <button
          class="btn"
          style="padding: 2px 6px;"
          :title="'Toggle follow PC'"
          @click="follow = !follow"
        >
          {{ follow ? "🎯" : "↺" }}
        </button>
      </div>
    </div>
    <div class="panel-body">
      <div v-for="row in rows" :key="row.addr">
        <div class="mem-row" :class="{ 'mem-row-pc': row.isPcRow }">
          <span class="mem-addr" style="cursor: pointer;" @click="onAddrClick(row.addr)">
            <span v-if="row.isPcRow" class="pc-marker" title="PC">◀</span>
            {{ row.addr.toString(16).padStart(8, "0") }}
          </span>
          <span
            v-for="(b, j) in row.bytes"
            :key="j"
            class="mem-byte"
            :class="{ 'mem-byte-pc': row.isPcRow && sim.pc.value >= row.addr + j && sim.pc.value < row.addr + j + 1 }"
          >
            {{ b.toString(16).padStart(2, "0") }}
          </span>
        </div>
        <div class="mem-row mem-row-alt">
          <span class="mem-addr" style="visibility: hidden;">
            {{ "00000000" }}
          </span>
          <span class="mem-word" v-for="(w, j) in row.words" :key="j">
            {{ w.toString(16).padStart(8, "0") }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
