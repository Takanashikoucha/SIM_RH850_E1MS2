<script setup lang="ts">
import { computed } from "vue";
import { injectSim } from "../composables/useSimulator";

const sim = injectSim();

const regNames = [
  "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7",
  "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
  "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23",
  "r24", "r25", "r26", "r27", "r28", "r29", "r30", "r31",
];

const specialRegs: Record<string, string> = {
  r0: "zero",
  r3: "SP",
  r4: "GP",
  r5: "TP",
  r30: "EP",
  r31: "LP",
};

const regs = computed(() => {
  return regNames.map((name, i) => ({
    name,
    special: specialRegs[name],
    value: sim.regs.value[i],
    changed: sim.changedRegs.value.has(i),
  }));
});

const pswFlags = computed(() => sim.pswFlags());
const pswHex = computed(() => sim.psw.value.toString(16).padStart(8, "0"));
const changedCount = computed(() => sim.changedRegs.value.size);

function editReg(idx: number) {
  const s = sim.sim.value;
  if (!s) return;
  const input = window.prompt(`Set ${regNames[idx]} (hex):`, s.getReg(idx).toString(16));
  if (input !== null) {
    const v = parseInt(input, 16) >>> 0;
    s.setReg(idx, v);
    sim.refreshState();
  }
}
</script>

<template>
  <div class="panel" style="flex: 1;">
    <div class="panel-title">
      General Registers
      <span v-if="changedCount > 0" style="margin-left: auto; font-weight: 400; text-transform: none; color: var(--yellow);">
        {{ changedCount }} changed
      </span>
    </div>
    <div class="panel-body">
      <div class="reg-grid">
        <div
          v-for="(reg, i) in regs"
          :key="reg.name"
          class="reg-cell"
          :class="{ changed: reg.changed }"
          @click="editReg(i)"
          :title="reg.special ? `${reg.name} = ${reg.special}` : 'Click to edit'"
        >
          <span class="reg-name">
            {{ reg.name }}
            <span v-if="reg.special" style="color: var(--purple); margin-left: 2px;">{{ reg.special }}</span>
          </span>
          <span class="reg-value">
            {{ reg.value.toString(16).padStart(8, "0") }}
          </span>
        </div>
      </div>
    </div>

    <!-- PSW -->
    <div class="panel-title" style="border-top: 1px solid var(--border);">
      PSW = 0x{{ pswHex }}
    </div>
    <div class="psw-flags">
      <span
        v-for="f in pswFlags"
        :key="f.name"
        class="psw-flag"
        :class="{ set: f.set }"
        :title="`bit ${f.bit}`"
      >
        {{ f.name }}
      </span>
    </div>
  </div>
</template>
