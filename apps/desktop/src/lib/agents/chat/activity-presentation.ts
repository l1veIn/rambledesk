// Tool lifecycle predicates adapted from Codeg src/lib/tool-call-lifecycle.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: authoritative generated content and legacy text share one presentation adapter.
import type { SessionContentBlock, SessionToolCall, SessionToolStatus } from '$lib/generated/feedback'
import type { SessionActivity } from '../managedSessionUi'
import { generateUnifiedDiff } from './unified-diff-generator'

export function toolStatusUnsettled(status: string | null | undefined): boolean {
  if (status == null) return false
  const normalized = status.trim().toLowerCase()
  return normalized !== 'completed' && normalized !== 'failed'
}

export function toolPresentation(status: SessionToolStatus, runActive: boolean) {
  const unsettled = toolStatusUnsettled(status)
  return {
    spinning: unsettled && runActive,
    incomplete: unsettled && !runActive,
    failed: status === 'failed',
    label: status === 'completed' ? 'Completed' : status === 'failed' ? 'Failed'
      : !runActive ? 'No final result' : status === 'pending' ? 'Pending' : 'Running',
  }
}

/** ACP raw fields may be bounded previews, so malformed/truncated JSON stays readable. */
export function formatToolJson(value: string | null): string {
  if (!value) return ''
  try { return JSON.stringify(JSON.parse(value), null, 2) } catch { return value }
}

export function activityTool(activity: SessionActivity): SessionToolCall | null {
  return activity.content?.type === 'tool_call' ? activity.content.tool : null
}

export function activityMessage(activity: SessionActivity): { blocks: readonly SessionContentBlock[]; truncated: boolean } {
  if (activity.content?.type === 'message') return activity.content
  return { blocks: activity.text ? [{ type: 'text', text: activity.text }] : [], truncated: false }
}

export function contentText(block: SessionContentBlock): string {
  switch (block.type) {
    case 'text': return block.text
    case 'diff': return generateUnifiedDiff(block.old_text ?? '', block.new_text, block.path) ?? block.new_text
    case 'resource': return [block.name || embeddedAttachmentName(block.uri) || block.uri, block.text].filter(Boolean).join('\n')
    case 'image': return block.uri || `[${block.mime_type}]`
    case 'audio': return `[${block.mime_type}]`
    case 'terminal': return `Terminal: ${block.terminal_id}`
    case 'unsupported': return block.label
  }
}

export function embeddedAttachmentName(uri: string): string | null {
  if (!uri.startsWith('ramble-attachment://')) return null
  try { return decodeURIComponent(new URL(uri).pathname.slice(1)) || null }
  catch { return null }
}

export function activityQuoteText(activity: SessionActivity): string {
  const tool = activityTool(activity)
  if (tool) return [tool.title, ...tool.content.map(contentText), tool.raw_output].filter(Boolean).join('\n\n')
  return activityMessage(activity).blocks.map(contentText).join('\n\n')
}

const IMAGE_TYPES = new Set(['image/png', 'image/jpeg', 'image/gif', 'image/webp'])
const AUDIO_TYPES = new Set(['audio/mpeg', 'audio/mp3', 'audio/wav', 'audio/x-wav', 'audio/ogg', 'audio/webm', 'audio/flac'])
const MAX_INLINE_BASE64 = 1_400_000

/** Inline media is decoded only from bounded raster/audio bytes; URIs remain explicit links. */
export function inlineMediaSource(block: Extract<SessionContentBlock, { type: 'image' | 'audio' }>): string | null {
  const allowed = block.type === 'image' ? IMAGE_TYPES : AUDIO_TYPES
  if (!allowed.has(block.mime_type) || !block.data || block.data.length > MAX_INLINE_BASE64 || !/^[A-Za-z0-9+/]*={0,2}$/.test(block.data)) return null
  return `data:${block.mime_type};base64,${block.data}`
}

export function locationLabel(location: { path: string; line: number | null }): string {
  return location.line === null ? location.path : `${location.path}:${location.line}`
}

export function latestStreamingActivity(activities: readonly SessionActivity[], runActive: boolean): string | null {
  if (!runActive) return null
  const latest = activities.at(-1)
  return latest && (latest.kind === 'agent_message' || latest.kind === 'agent_thought') ? latest.id : null
}

/** A later turn must not animate an unfinished tool left in an earlier turn. */
export function activityInRunningTurn(activity: SessionActivity, activities: readonly SessionActivity[], runActive: boolean): boolean {
  if (!runActive) return false
  const currentTurn = activities.findLast((entry) => entry.turn_id != null)?.turn_id
  return currentTurn ? activity.turn_id === currentTurn : activities.at(-1)?.id === activity.id
}

export function activityHasQuote(activity: SessionActivity): boolean {
  const tool = activityTool(activity)
  if (tool) return Boolean(tool.title || tool.content.length || tool.raw_output)
  return activityMessage(activity).blocks.some((block) => block.type !== 'text' || block.text.trim())
}
