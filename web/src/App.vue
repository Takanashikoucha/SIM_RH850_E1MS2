<script setup lang="ts">
import { onMounted, onUnmounted, computed, ref } from "vue";
import { useSimulator } from "./composables/useSimulator";
import ControlBar from "./components/ControlBar.vue";
import CodeView from "./components/CodeView.vue";
import RegisterPanel from "./components/RegisterPanel.vue";
import MemoryPanel from "./components/MemoryPanel.vue";
import WatchPanel from "./components/WatchPanel.vue";
import TracePanel from "./components/TracePanel.vue";
import CallStackPanel from "./components/CallStackPanel.vue";
import StatusBar from "./components/StatusBar.vue";
import FaultBanner from "./components/FaultBanner.vue";

const sim = useSimulator();
const fileInput = ref<HTMLInputElement | null>(null);

const statusPill = computed(() => {
  if (sim.halted.value) return { cls: "halted", text: "HALTED" };
  if (sim.running.value) return { cls: "running", text: "RUNNING" };
  if (sim.stopped.value) return { cls: "stopped", text: sim.stopReason.value || "STOPPED" };
  return { cls: "ready", text: "READY" };
});

const rbaseInput = computed({
  get: () => sim.rbaseInput.value,
  set: (v: string) => { sim.rbaseInput.value = v; },
});

const isRunning = computed(() => sim.running.value);
const fault = computed(() => sim.fault.value);
const errorMsg = computed(() => {
  const e = sim.error.value;
  return e && e.trim() ? e : null;
});
const lstCount = computed(() => (sim.lstLoaded.value ? sim.lstLines.value.length : 0));
const symCount = computed(() => sim.symbols.value.length);

function onKeydown(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement) return;
  switch (e.key) {
    case "F5":
      e.preventDefault();
      sim.doRun();
      break;
    case "F10":
      e.preventDefault();
      sim.doStep();
      break;
    case "F11":
      e.preventDefault();
      sim.doStepOver();
      break;
    case "Escape":
      sim.doStop();
      break;
    case "r":
      sim.doReset();
      break;
  }
}

function openFilePicker() {
  fileInput.value?.click();
}

function onFilePicker(e: Event) {
  const input = e.target as HTMLInputElement;
  const files = input.files;
  if (!files) return;
  for (const f of files) {
    handleFile(f);
  }
  input.value = ""; // allow re-selecting the same file
}

function handleFile(f: File) {
  const name = f.name.toLowerCase();
  if (name.endsWith(".bin") || name.endsWith(".rom")) {
    const reader = new FileReader();
    reader.onload = () => {
      sim.loadRom(new Uint8Array(reader.result as ArrayBuffer), 0);
    };
    reader.readAsArrayBuffer(f);
  } else if (name.endsWith(".lst") || name.endsWith(".list")) {
    const reader = new FileReader();
    reader.onload = () => {
      sim.loadListing(reader.result as string);
    };
    reader.readAsText(f);
  } else if (name.endsWith(".map")) {
    const reader = new FileReader();
    reader.onload = () => {
      sim.loadMap(reader.result as string);
    };
    reader.readAsText(f);
  }
}

function onFileDrop(e: DragEvent) {
  e.preventDefault();
  const files = e.dataTransfer?.files;
  if (!files) return;
  for (const f of files) {
    handleFile(f);
  }
}

onMounted(() => {
  sim.init();
  window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div @dragover.prevent @drop="onFileDrop">
    <!-- Header -->
    <header class="header">
      <span class="logo">◆</span>
      <h1>
        RH850/E1M-S2 <span class="sub">Execution Simulator</span>
      </h1>
      <div class="spacer" />
      <!-- File picker -->
      <input
        ref="fileInput"
        type="file"
        multiple
        accept=".bin,.rom,.lst,.list,.map"
        style="display: none;"
        @change="onFilePicker"
      />
      <button class="btn primary" @click="openFilePicker()" title="Open .bin / .rom / .lst / .map files">
        📂 Open
      </button>
      <span v-if="lstCount" style="font-size: 11px; color: var(--green); margin-left: 4px;">
        lst:{{ lstCount }}
      </span>
      <span v-if="symCount" style="font-size: 11px; color: var(--green); margin-left: 4px;">
        sym:{{ symCount }}
      </span>
      <div style="width: 1px; height: 20px; background: var(--border); margin: 0 8px;" />
      <input
        class="input"
        style="width: 90px;"
        v-model="rbaseInput"
        placeholder="RBASE"
        @keyup.enter="sim.applyRbase()"
      />
      <button class="btn" @click="sim.applyRbase()" title="Set RBASE and reset">Set</button>
      <span class="status-pill" :class="statusPill.cls">
        <span v-if="isRunning" class="pulse" />
        {{ statusPill.text }}
      </span>
    </header>

    <!-- Main -->
    <main class="main">
      <!-- Controls -->
      <ControlBar />

      <!-- Fault banner -->
      <div v-if="fault" style="padding: 0 6px 6px 6px;">
        <FaultBanner :lastPc="fault.lastPc" :cause="fault.cause" />
      </div>

      <!-- Transient error toast (only when there's actual message text) -->
      <div v-if="errorMsg" style="padding: 0 6px 6px 6px;">
        <div class="error-toast" @click="sim.error.value = null">
          <span class="error-icon">⚠</span>
          <span>{{ errorMsg }}</span>
        </div>
      </div>

      <!-- Left -->
      <div class="left-col">
        <MemoryPanel />
      </div>

      <!-- Center -->
      <div class="center-col">
        <CallStackPanel />
        <CodeView />
        <WatchPanel />
      </div>

      <!-- Right -->
      <div class="right-col">
        <RegisterPanel />
        <TracePanel />
      </div>

      <!-- Status bar -->
      <StatusBar />
    </main>
  </div>
</template>

<style>
@import "./style.css";
</style>
