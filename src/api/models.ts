import { del, get, post, put } from './client';
import type { ModelProvider } from '../constants/providers';

export type ModelType = 'openai' | 'anthropic';
export type CatalogModel = {
  id: string;
  name: string;
  type: ModelType;
  provider: ModelProvider;
  is_default: number;
};

export type ModelPayload = Pick<CatalogModel, 'name' | 'type' | 'provider'>;

export type ModelListParams = {
  type?: ModelType;
  provider?: ModelProvider;
};

export const modelsApi = {
  list: (filters: ModelListParams = {}) => {
    const params = new URLSearchParams();
    if (filters.type) params.set('type', filters.type);
    if (filters.provider) params.set('provider', filters.provider);
    const query = params.toString();
    return get<{ items: CatalogModel[] }>(`/api/models${query ? `?${query}` : ''}`);
  },
  create: (value: ModelPayload) => post<CatalogModel>('/api/models', value),
  update: (id: string, value: ModelPayload) => put<CatalogModel>(`/api/models/${id}`, value),
  remove: (id: string) => del<void>(`/api/models/${id}`),
  setDefault: (id: string) => post<CatalogModel>(`/api/models/${id}/set-default`),
};
