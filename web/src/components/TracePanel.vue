<script setup lang="ts">
import { computed } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();

const traceCount = computed(() => sim.trace.value.length);

const entries = computed(() => {
  const s = sim.sim.value;
  if (!s) return [];
  return sim.trace.value.slice(-20).reverse().map((t) => {
    const h0 = s.getMemWord(t.pc) & 0xffff;
    const text = disasm16(h0);
    return { ...t, text };
  });
});

function disasm16(h0: number): string {
  const op6 = (h0 >> 5) & 0x3f;
  const r2 = (h0 >> 11) & 0x1f;
  const r1 = h0 & 0x1f;
  switch (op6) {
    case 0x00: return "NOP";
    case 0x01: return `NOT r${r2}`;
    case 0x03: return `JMP [r${r1}]`;
    case 0x06: return `OR r${r2}, r${r1}`;
    case 0x07: return `XOR r${r2}, r${r1}`;
    case 0x0A: return `AND r${r2}, r${r1}`;
    case 0x0D: return `SUB r${r2}, r${r1}`;
    case 0x0E: return `ADD r${r2}, r${r1}`;
    case 0x0F: return `CMP r${r2}, r${r1}`;
    case 0x1F: return `EXT`;
    case 0x20: return `BZ`;
    case 0x21: return `BNZ`;
    case 0x30: return `ADDI r${r2}`;
    case 0x32: return `MOVHI r${r2}`;
    default: return `.word ${h0.toString(16).padStart(4, "0")}`;
  }
}
</script>

<template>
  <div class="panel" style="flex-shrink: 0;">
    <div class="panel-title">
      Instruction Trace
      <span style="margin-left: auto; font-weight: 400; text-transform: none;">
        {{ traceCount }} entries
      </span>
    </div>
    <div class="panel-body" style="max-height: 150px;">
      <div v-if="traceCount === 0" style="padding: 8px; color: var(--text-dim); font-size: 11px;">
        Step or run to trace instructions
      </div>
      <div v-for="(e, i) in entries" :key="i" class="trace-row">
        <span class="t-ins">{{ e.ins }}</span>
        <span class="t-pc" @click="sim.seek(e.pc)">0x{{ e.pc.toString(16).padStart(8, "0") }}</span>
        <span class="t-info">{{ e.text }} <span v-if="e.changedRegs > 0" style="color: var(--yellow);">+{{ e.changedRegs }}R</span></span>
      </div>
    </div>
  </div>
</template>
