import { createApp } from 'vue';
import { createPinia } from 'pinia';
import ElementPlus from 'element-plus';
import 'element-plus/dist/index.css';
import './styles/index.scss';
import router from './router';
import App from './App.vue';
import { invoke } from '@tauri-apps/api/core';
import { ElMessageBox } from 'element-plus';
import { setApiBase } from './api/client';

const configureTauriGateway = async (): Promise<boolean> => {
  try {
    setApiBase(await invoke<string>('gateway_url'));
    return true;
  } catch {
    // 浏览器直接运行前端时，继续使用 VITE_API_BASE 或默认 7789 端口。
    return false;
  }
};

const bindDevtoolsShortcut = () => {
  window.addEventListener('keydown', (event) => {
    const isDevtoolsShortcut =
      event.key === 'F12' || (event.ctrlKey && event.shiftKey && event.key === 'I');
    if (!isDevtoolsShortcut) return;
    event.preventDefault();
    invoke('open_devtools').catch(() => undefined);
  });
};

const bindCloseHandler = () => {
  let handling = false;
  window.addEventListener('aimux-close-requested', () => {
    void (async () => {
      if (handling) return;
      handling = true;
      try {
        await ElMessageBox.confirm(
          '直接退出会停止网关、账号监控和倍率监控。选择最小化后，AIMux 将继续在系统托盘运行。',
          '关闭 AIMux',
          {
            confirmButtonText: '直接退出',
            cancelButtonText: '最小化到托盘',
            closeOnClickModal: false,
            closeOnPressEscape: false,
            showClose: false,
          },
        );
        await invoke('exit_app');
      } catch (reason) {
        if (reason === 'cancel') await invoke('minimize_to_tray');
      } finally {
        handling = false;
      }
    })();
  });
};

const bootstrap = async () => {
  if (await configureTauriGateway()) {
    bindDevtoolsShortcut();
    createApp(App).use(createPinia()).use(router).use(ElementPlus).mount('#app');
    bindCloseHandler();
    return;
  }
  createApp(App).use(createPinia()).use(router).use(ElementPlus).mount('#app');
};

void bootstrap();
