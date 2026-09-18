import alibabaLogo from '../assets/providers/alibaba.svg';
import anthropicLogo from '../assets/providers/anthropic.svg';
import deepseekLogo from '../assets/providers/deepseek.svg';
import googleLogo from '../assets/providers/google.svg';
import moonshotLogo from '../assets/providers/moonshot.svg';
import openaiLogo from '../assets/providers/openai.svg';
import xaiLogo from '../assets/providers/xai.svg';
import zhipuLogo from '../assets/providers/zhipu.svg';

export const PROVIDERS = [
  { value: 'openai', label: 'OpenAI', logo: openaiLogo },
  { value: 'anthropic', label: 'Anthropic', logo: anthropicLogo },
  { value: 'xai', label: 'xAI', logo: xaiLogo },
  { value: 'google', label: 'Google', logo: googleLogo },
  { value: 'deepseek', label: 'DeepSeek', logo: deepseekLogo },
  { value: 'moonshot', label: 'Moonshot AI', logo: moonshotLogo },
  { value: 'alibaba', label: 'Alibaba Cloud', logo: alibabaLogo },
  { value: 'zhipu', label: '智谱 AI', logo: zhipuLogo },
] as const;

export type ModelProvider = (typeof PROVIDERS)[number]['value'];

export const providerMeta = (value: string) =>
  PROVIDERS.find((provider) => provider.value === value);
