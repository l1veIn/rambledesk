import { describe, expect, expectTypeOf, it } from 'vitest'

import type {
  DraftView,
  FeedbackPackageContent,
  FeedbackRequestSummary,
  FeedbackWorkspaceView,
  GetFeedbackInput,
  ListFeedbackRequestsInput,
} from '../generated/feedback'
import type { ContinuationMode, HostAdapter, HostProfile } from '../generated/hosts'
import {
  isApplicationError,
  isApplicationErrorCode,
  type ApplicationCommandInput,
  type ApplicationCommandName,
  type ApplicationCommandResult,
  type ReadAttachmentBytesInput,
} from './contracts'

describe('application command contracts', () => {
  it('binds semantic command names to domain inputs and results', () => {
    expectTypeOf<ApplicationCommandInput<'listFeedbackInbox'>>().toEqualTypeOf<undefined>()
    expectTypeOf<ApplicationCommandResult<'listFeedbackInbox'>>().toEqualTypeOf<
      FeedbackRequestSummary[]
    >()
    expectTypeOf<ApplicationCommandInput<'listHostProfiles'>>().toEqualTypeOf<undefined>()
    expectTypeOf<ApplicationCommandResult<'listHostProfiles'>>().toEqualTypeOf<HostProfile[]>()
    expectTypeOf<ApplicationCommandInput<'listFeedbackRequests'>>().toEqualTypeOf<
      ListFeedbackRequestsInput
    >()
    expectTypeOf<ApplicationCommandInput<'getFeedbackWorkspace'>>().toEqualTypeOf<
      GetFeedbackInput
    >()
    expectTypeOf<ApplicationCommandResult<'getFeedbackWorkspace'>>().toEqualTypeOf<
      FeedbackWorkspaceView
    >()
    expectTypeOf<ApplicationCommandResult<'readPublishedFeedback'>>().toEqualTypeOf<
      FeedbackPackageContent | null
    >()
    expectTypeOf<FeedbackPackageContent['manifest']['source_revision']>().toEqualTypeOf<number>()
    expectTypeOf<FeedbackPackageContent['manifest']['draft_revision']>().toEqualTypeOf<number>()
    expectTypeOf<
      FeedbackPackageContent['manifest']['attachments'][number]['byte_size']
    >().toEqualTypeOf<number>()
    expectTypeOf<ApplicationCommandResult<'saveFeedbackDraft'>>().toEqualTypeOf<DraftView>()
    expectTypeOf<ApplicationCommandInput<'readFeedbackAttachment'>>().toEqualTypeOf<
      ReadAttachmentBytesInput
    >()
    expectTypeOf<ApplicationCommandResult<'readFeedbackAttachment'>>().toEqualTypeOf<ArrayBuffer>()
    expectTypeOf<HostAdapter>().toEqualTypeOf<'generic_mcp' | 'pi_native'>()
    expectTypeOf<ContinuationMode>().toEqualTypeOf<'not_required' | 'manual' | 'native'>()
  })

  it('contains only the intended cross-client operation names', () => {
    const commands = [
      'listFeedbackInbox',
      'listHostSessions',
      'listArchivedHostSessions',
      'listHostProfiles',
      'listFeedbackRequests',
      'getFeedbackWorkspace',
      'readPublishedFeedback',
      'saveFeedbackDraft',
      'addFeedbackAttachment',
      'removeFeedbackAttachment',
      'reorderFeedbackAttachments',
      'submitFeedback',
      'approveFeedbackRequest',
      'cancelFeedbackRequest',
      'renameHostSession',
      'setHostSessionPinned',
      'archiveHostSession',
      'unarchiveHostSession',
      'deleteHostSession',
      'deleteFeedbackRequest',
      'setHostPinned',
      'readFeedbackAttachment',
      'readRequestAttachment',
    ] as const satisfies readonly ApplicationCommandName[]

    expect(commands).toHaveLength(23)
    expectTypeOf<(typeof commands)[number]>().toEqualTypeOf<ApplicationCommandName>()
  })
})

describe('application error guards', () => {
  it('accepts the generated structured error contract', () => {
    const error = {
      code: 'DRAFT_CONFLICT',
      message: 'draft revision changed; reload before saving or submitting',
      retryable: false,
    }

    expect(isApplicationError(error)).toBe(true)
    expect(isApplicationErrorCode(error.code)).toBe(true)
  })

  it('rejects unknown codes and malformed payloads', () => {
    expect(isApplicationErrorCode('TAURI_FAILURE')).toBe(false)
    expect(isApplicationError({ code: 'TAURI_FAILURE', message: 'failed', retryable: true })).toBe(
      false,
    )
    expect(isApplicationError({ code: 'STORAGE_FAILURE', message: 'failed' })).toBe(false)
    expect(isApplicationError('STORAGE_FAILURE')).toBe(false)
  })
})
