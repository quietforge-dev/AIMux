import { computed, ref } from 'vue';
import type { TableColumnCtx } from 'element-plus';

const MIN_WIDTH = 40;
const MAX_WIDTH = 1200;

const validWidth = (value: unknown): value is number =>
  typeof value === 'number' && Number.isInteger(value) && value >= MIN_WIDTH && value <= MAX_WIDTH;

const readWidths = (storageKey: string): Record<string, number> => {
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(([key, value]) => key && validWidth(value)),
    );
  } catch {
    return {};
  }
};

export const useTableColumnWidths = (tableId: string) => {
  const storageKey = `aimux.ui.table-widths.v1.${tableId}`;
  const widths = ref<Record<string, number>>(readWidths(storageKey));
  const tableKey = ref(0);
  const hasCustomWidths = computed(() => Object.keys(widths.value).length > 0);

  const columnWidth = (columnKey: string, defaultWidth?: number) =>
    widths.value[columnKey] ?? defaultWidth;

  const handleColumnResize = (newWidth: number, _oldWidth: number, column: TableColumnCtx) => {
    if (!column.columnKey) return;
    const width = Math.round(newWidth);
    if (!validWidth(width)) return;
    widths.value = { ...widths.value, [column.columnKey]: width };
    try {
      window.localStorage.setItem(storageKey, JSON.stringify(widths.value));
    } catch {
      // 存储不可用时仍保留本次页面内的调整。
    }
  };

  const resetColumnWidths = () => {
    widths.value = {};
    try {
      window.localStorage.removeItem(storageKey);
    } catch {
      // 存储不可用不影响恢复当前页面的默认列宽。
    }
    tableKey.value += 1;
  };

  return { tableKey, hasCustomWidths, columnWidth, handleColumnResize, resetColumnWidths };
};
