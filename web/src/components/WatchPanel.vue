<script setup lang="ts">
import { computed, ref } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();
const addrInput = ref("00000000");
const showSymbols = ref(false);

function addEntry() {
  const v = parseInt(addrInput.value, 16) >>> 0;
  sim.addWatch(v);
}

const entries = computed(() => {
  const s = sim.sim.value;
  if (!s) return [];
  return sim.watch.value.map((w, idx) => {
    const raw = s.readWord(w.addr);
    return {
      ...w,
      idx,
      hex: raw.toString(16).padStart(8, "0"),
      dec: raw,
    };
  });
});

const watchCount = computed(() => sim.watch.value.length);
const symbolCount = computed(() => sim.symbols.value.length);

function addSymbol(sym: { name: string; addr: number }) {
  sim.watchSymbol(sym.name, sym.addr);
}
</script>

<template>
  <div class="panel" style="flex-shrink: 0;">
    <div class="panel-title">
      Data Watch
      <div style="margin-left: auto; display: flex; gap: 4px; align-items: center;">
        <input
          class="input"
          style="width: 80px;"
          v-model="addrInput"
          placeholder="addr hex"
          @keyup.enter="addEntry()"
        />
        <button class="btn" style="padding: 2px 6px;" @click="addEntry()">+</button>
        <button
          v-if="symbolCount > 0"
          class="btn"
          style="padding: 2px 6px;"
          :title="`Show/hide ${symbolCount} map symbols`"
          @click="showSymbols = !showSymbols"
        >
          {{ showSymbols ? "✕ sym" : `◆ sym (${symbolCount})` }}
        </button>
      </div>
    </div>
    <div class="panel-body" style="max-height: 160px;">
      <!-- Symbol list (toggleable) -->
      <div v-if="showSymbols && symbolCount > 0" class="sym-list">
        <div class="sym-header">
          <span>Name</span>
          <span>Address</span>
          <span>Size</span>
          <span></span>
        </div>
        <div
          v-for="sym in sim.symbols.value"
          :key="sym.name + sym.addr"
          class="sym-row"
        >
          <span class="sym-name">{{ sym.name }}</span>
          <span class="sym-addr" @click="sim.seek(sym.addr)">0x{{ sym.addr.toString(16).padStart(8, "0") }}</span>
          <span class="sym-size">{{ sym.size || "—" }}</span>
          <button class="btn" style="padding: 1px 4px; font-size: 10px;" @click="addSymbol(sym)" title="Add to watch">+</button>
        </div>
      </div>
      <!-- Watch entries -->
      <div v-if="watchCount === 0 && (!showSymbols || symbolCount === 0)" style="padding: 8px; color: var(--text-dim); font-size: 11px;">
        Add watch addresses or load a .map file for symbols
      </div>
      <div v-for="e in entries" :key="e.idx" class="watch-row">
        <span class="w-addr" @click="sim.seek(e.addr)">0x{{ e.hex }}</span>
        <span class="w-label">{{ e.label }}</span>
        <span class="w-value">
          <span style="cursor: pointer;" @click="() => sim.removeWatch(e.idx)" title="Remove">✕</span>
          &nbsp;{{ e.hex }} &nbsp; {{ e.dec }}
        </span>
      </div>
    </div>
  </div>
</template>
