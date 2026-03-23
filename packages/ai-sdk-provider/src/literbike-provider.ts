import { createOpenAICompatible } from "@ai-sdk/openai-compatible"
import type { LanguageModelV2 } from "@ai-sdk/provider"

export interface LiterbikeProviderSettings {
  baseURL?: string
  apiKey?: string
  headers?: Record<string, string>
  fetch?: typeof fetch
}

export interface LiterbikeProvider {
  (modelId: string): LanguageModelV2
  languageModel(modelId: string): LanguageModelV2
  chat(modelId: string): LanguageModelV2
}

export function createLiterbike(options?: LiterbikeProviderSettings): LiterbikeProvider {
  const inner = createOpenAICompatible({
    name: "literbike",
    baseURL: options?.baseURL ?? "http://localhost:8888/v1",
    apiKey: options?.apiKey ?? "literbike",
    headers: options?.headers,
    fetch: options?.fetch,
  })

  const provider = (modelId: string) => inner(modelId)
  provider.languageModel = (modelId: string) => inner.languageModel(modelId)
  provider.chat = (modelId: string) => inner.languageModel(modelId)

  return provider as LiterbikeProvider
}

export const literbike = createLiterbike()
