<script setup lang="ts">
import { computed, ref, onMounted, watch } from "vue";
import { injectSim } from "../composables/useSimulator";
import { disasm, type DisasmLine } from "../disasm";

const sim = injectSim();
const container = ref<HTMLElement | null>(null);

const pcHex = computed(() => sim.pc.value.toString(16).padStart(8, "0"));
const romLoaded = computed(() => sim.romLoaded.value);

// Build an addr→lstLine lookup for O(1) source comment retrieval
const lstMap = computed(() => {
  const m = new Map<number, { asm: string; src: string }>();
  for (const l of sim.lstLines.value) {
    m.set(l.addr, { asm: l.asm, src: l.src });
  }
  return m;
});

const lines = computed((): (DisasmLine & { src?: string; symbol?: string })[] => {
  const s = sim.sim.value;
  if (!s || !sim.romLoaded.value) return [];
  const pc = sim.pc.value;
  const romBase = sim.romBase.value;
  const romLen = sim.romLen.value;
  // Show 60 instructions centered around PC (30 before, 30 after)
  const start = Math.max(romBase, pc - 48);
  const end = Math.min(romBase + romLen, start + 60 * 8); // max 480 bytes
  // Build a view of the code region
  const size = end - start;
  if (size <= 0) return [];
  const buf = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    buf[i] = s.getMemByte(start + i) & 0xff;
  }
  const lmap = lstMap.value;
  const out: (DisasmLine & { src?: string; symbol?: string })[] = [];
  let pos = 0;
  while (pos < size - 1) {
    const res = disasm(buf, pos);
    const addr = start + pos;
    const lst = lmap.get(addr);
    const sym = sim.symbolAt(addr);
    out.push({
      addr,
      text: res.text,
      bytes: res.bytes,
      kind: res.kind,
      src: lst?.src,
      symbol: sym?.name,
    });
    pos += res.bytes;
    if (out.length >= 60) break;
  }
  return out;
});

// Color classes by instruction kind
const kindClass: Record<string, string> = {
  arith: "inst-arith",
  logic: "inst-logic",
  load: "inst-load",
  store: "inst-store",
  branch: "inst-branch",
  call: "inst-call",
  ret: "inst-ret",
  system: "inst-system",
  fpu: "inst-fpu",
  data: "inst-data",
  unknown: "inst-unknown",
};

function onLineClick(addr: number) {
  sim.toggleBp(addr);
}

function scrollToPc() {
  const el = container.value;
  if (!el || !sim.autoFollow.value) return;
  const current = el.querySelector(".code-line.current");
  if (current) {
    current.scrollIntoView({ block: "center", behavior: "smooth" });
  }
}

watch(() => sim.pc.value, () => {
  requestAnimationFrame(scrollToPc);
});

onMounted(() => {
  scrollToPc();
});
</script>

<template>
  <div class="panel" style="flex: 1;">
    <div class="panel-title">
      Disassembly
      <span style="margin-left: auto; font-weight: 400; text-transform: none; font-size: 11px;">
        PC = 0x{{ pcHex }}
      </span>
    </div>
    <div ref="container" class="panel-body code-view">
      <div v-if="!romLoaded" style="padding: 16px; color: var(--text-dim); text-align: center;">
        Drop a .bin / .rom file to load ROM
      </div>
      <div
        v-for="line in lines"
        :key="line.addr"
        class="code-line"
        :class="[kindClass[line.kind] || '', { current: line.addr === sim.pc.value }]"
        @click="onLineClick(line.addr)"
      >
        <span class="addr">
          <span v-if="sim.breakpoints.value.has(line.addr)" class="bp-dot">●</span>
          {{ line.addr.toString(16).padStart(8, "0") }}
        </span>
        <span class="mnem">{{ line.text }}</span>
        <span class="src-comment" v-if="line.src">{{ line.src }}</span>
        <span class="bytes" v-if="line.bytes > 2">{{ line.bytes }}B</span>
        <span class="sym-name" v-if="line.symbol" :title="`Symbol: ${line.symbol}`">{{ line.symbol }}</span>
      </div>
    </div>
  </div>
</template>
