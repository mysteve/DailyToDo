<script setup lang="ts">
import { ref } from 'vue'
import { CheckCircle2, Database, Download, KeyRound, LoaderCircle, Monitor, Sparkles, Upload } from '@lucide/vue'
import { api, desktop } from '../api'
import { busy, mutate, notify, workspace } from '../store'
import ModalShell from './ModalShell.vue'

const emit = defineEmits<{ close: [] }>()
const endpoint = ref(workspace.value.settings.endpoint)
const model = ref(workspace.value.settings.model)
const apiKey = ref('')
const clearKey = ref(false)
const error = ref('')
const testing = ref(false)
const testResult = ref('')
const backupBusy = ref(false)

async function save(close = true) {
  error.value = ''
  if (!/^https?:\/\//.test(endpoint.value.trim())) { error.value = '请输入完整接口地址'; return false }
  const success = await mutate(() => api.saveSettings({ endpoint: endpoint.value.trim(), model: model.value.trim(), hasApiKey: workspace.value.settings.hasApiKey }, apiKey.value, clearKey.value))
  if (success) { apiKey.value = ''; clearKey.value = false; if (close) { notify('设置已保存'); emit('close') } }
  return success
}

async function test() {
  if (!model.value.trim()) { error.value = '请填写模型名称'; return }
  if (!await save(false)) return
  testing.value = true
  testResult.value = ''
  try { testResult.value = await api.testAi() } catch (err) { error.value = String(err) }
  finally { testing.value = false }
}

async function backup(importing: boolean) {
  backupBusy.value = true
  try {
    if (importing) { await mutate(async () => { if (await api.importBackup()) notify('备份已合并，已有记录保持原样') }) }
    else if (await api.exportBackup()) notify('备份已导出')
  } catch (err) { notify(String(err)) }
  finally { backupBusy.value = false }
}
</script>

<template>
  <ModalShell title="数据与设置" @close="emit('close')"><form class="modal-body settings-body" @submit.prevent="save()"><div class="settings-section-heading"><Sparkles :size="18" /><h3>AI 服务</h3><span class="status-pill" :class="{ configured: workspace.settings.model }">{{ workspace.settings.model ? '已配置' : '未配置' }}</span></div><div class="service-presets"><button class="text-button" type="button" @click="endpoint = 'https://api.openai.com/v1'">兼容 OpenAI</button><span>·</span><button class="text-button" type="button" @click="endpoint = 'http://127.0.0.1:11434/v1'">本地 Ollama</button></div><label class="field-label" for="ai-endpoint">接口地址</label><input id="ai-endpoint" v-model="endpoint" type="url" maxlength="1000" placeholder="https://your-service.com/v1" required /><label class="field-label" for="ai-model">模型名称</label><input id="ai-model" v-model="model" maxlength="200" placeholder="填写服务支持的模型名称" /><label class="field-label" for="ai-key">API Key <span>本地服务可留空</span></label><div class="key-input"><KeyRound :size="16" /><input id="ai-key" v-model="apiKey" type="password" maxlength="8192" autocomplete="off" :disabled="!desktop || clearKey" :placeholder="workspace.settings.hasApiKey ? '已保存密钥 · 留空保持' : '填写你的 API Key'" /></div><p class="field-help">{{ desktop ? '密钥保存在系统凭据库。修改接口地址时，请重新填写密钥。' : '浏览器仅供界面预览，AI 服务请在桌面应用中配置。' }}</p><label v-if="workspace.settings.hasApiKey" class="checkbox-label"><input v-model="clearKey" type="checkbox" />清除已保存的密钥</label><div class="connection-row"><button type="button" class="button secondary compact" :disabled="!desktop || testing || busy" @click="test"><LoaderCircle v-if="testing" class="spin" :size="14" /><Monitor v-else :size="14" />{{ testing ? '测试中…' : '保存并测试连接' }}</button><span v-if="testResult" class="connection-success"><CheckCircle2 :size="14" />{{ testResult }}</span></div><p v-if="error" class="inline-error" role="alert">{{ error }}</p>
      <div class="settings-divider" /><div class="settings-section-heading"><Database :size="18" /><h3>本地数据</h3></div><div class="storage-path">{{ workspace.storagePath }}</div><p class="field-help">备份包含手记、图片、待办和报告，不包含 AI 密钥。</p><div class="backup-actions"><button type="button" class="button secondary compact" :disabled="backupBusy || busy" @click="backup(false)"><Download :size="15" />导出备份</button><button type="button" class="button secondary compact" :disabled="backupBusy || busy" @click="backup(true)"><Upload :size="15" />导入备份</button></div><footer class="modal-footer"><span class="subtle">日迹 0.1.0</span><button class="button primary" type="submit" :disabled="busy || testing"><LoaderCircle v-if="busy" class="spin" :size="15" />保存设置</button></footer></form></ModalShell>
</template>
