<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();

interface Frame {
  retAddr: number;
  targetAddr: number;
  depth: number;
}

const frames = ref<Frame[]>([]);
const depth = computed(() => frames.value.length);

watch(
  () => sim.pc.value,
  (newPc, oldPc) => {
    if (oldPc === undefined || oldPc === null) return;
    const s = sim.sim.value;
    if (!s) return;
    let changed = false;
    while (frames.value.length > 0 && frames.value[frames.value.length - 1].retAddr === newPc) {
      frames.value.pop();
      changed = true;
    }
    if (changed) return;
    const h0 = s.getMemWord(oldPc) & 0xffff;
    const op6 = (h0 >> 5) & 0x3f;
    const r2 = (h0 >> 11) & 0x1f;
    let isCall = false;
    let target = 0;
    if (op6 === 0b000011) {
      isCall = true;
      target = s.getReg(r2) >>> 0;
    } else if (op6 === 0b11110 && r2 !== 0) {
      isCall = true;
      const h1 = s.getMemWord(oldPc + 2) & 0xffff;
      const disp = signExtend((h0 & 0x7ff) | (h1 << 11), 22) * 2;
      target = ((oldPc + 4 + disp) >>> 0) & 0xffffffff;
    } else if (op6 === 0b111101) {
      isCall = true;
      const h1 = s.getMemWord(oldPc + 2) & 0xffff;
      const disp = signExtend((h0 & 0x7ff) | (h1 << 11), 22) * 2;
      target = ((oldPc + 4 + disp) >>> 0) & 0xffffffff;
    }
    if (isCall && target === newPc) {
      if (frames.value.length === 0 || frames.value[frames.value.length - 1].targetAddr !== target) {
        frames.value.push({ retAddr: oldPc + 4, targetAddr: target, depth: frames.value.length });
      }
    }
  }
);

function signExtend(v: number, bits: number): number {
  const mask = 1 << bits;
  if (v & (mask >> 1)) return v - mask;
  return v;
}
</script>

<template>
  <div class="panel" style="flex-shrink: 0;">
    <div class="panel-title">
      Call Stack
      <span style="margin-left: auto; font-weight: 400; text-transform: none;">
        depth {{ depth }}
      </span>
    </div>
    <div class="panel-body" style="max-height: 80px;">
      <div v-if="depth === 0" style="padding: 4px 8px; color: var(--text-dim); font-size: 11px;">
        (empty)
      </div>
      <div
        v-for="(f, i) in frames"
        :key="i"
        class="stack-row"
        @click="sim.seek(f.targetAddr)"
      >
        <span class="depth">{{ f.depth }}</span>
        <span class="target">0x{{ f.targetAddr.toString(16).padStart(8, "0") }}</span>
        <span class="ret">→ 0x{{ f.retAddr.toString(16).padStart(8, "0") }}</span>
      </div>
    </div>
  </div>
</template>
