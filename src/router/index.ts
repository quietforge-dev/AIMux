import { createRouter, createWebHashHistory } from 'vue-router';
import MainLayout from '../layouts/MainLayout.vue';
import AccountsPage from '../pages/accounts/AccountsPage.vue';
import ModelsPage from '../pages/models/ModelsPage.vue';
import UsagePage from '../pages/usage/UsagePage.vue';
import StatisticsPage from '../pages/statistics/StatisticsPage.vue';
import MonitorPage from '../pages/monitor/MonitorPage.vue';
import MultiplierMonitorPage from '../pages/multiplier-monitor/MultiplierMonitorPage.vue';
import SettingsPage from '../pages/settings/SettingsPage.vue';

export default createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      component: MainLayout,
      redirect: '/accounts',
      children: [
        { path: 'accounts', component: AccountsPage },
        { path: 'models', component: ModelsPage },
        { path: 'usage', component: UsagePage },
        { path: 'statistics', component: StatisticsPage },
        { path: 'monitor', component: MonitorPage },
        { path: 'multiplier-monitor', component: MultiplierMonitorPage },
        { path: 'settings', component: SettingsPage },
      ],
    },
  ],
});
