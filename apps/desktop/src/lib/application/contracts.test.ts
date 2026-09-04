import { describe, expect, expectTypeOf, it } from 'vitest'

import type {
  AgentConfig,
  AgentConfigInput,
  AgentConnectionCheck,
  ApplicationFeedbackWorkspaceView,
  ApplicationHostProfileView,
  DraftView,
  FeedbackPackageView,
  FeedbackRequestSummary,
  GetFeedbackInput,
  ListFeedbackRequestsInput,
  ReadAttachmentInput,
  CreateManagedSessionInput,
  ManagedSessionInput,
  ManagedSessionSnapshot,
  SaveAgentConfigInput,
  SendManagedPromptInput,
  RespondManagedPermissionInput,
  ResolveFeedbackDeliveryInput,
} from '../generated/feedback'
import {
  isApplicationError,
  isApplicationErrorCode,
  type ApplicationAddAttachmentInput,
  type ApplicationCommandInput,
  type ApplicationCommandName,
  type ApplicationCommandResult,
} from './contracts'

describe('application command contracts', () => {
  it('binds semantic command names to domain inputs and results', () => {
    expectTypeOf<ApplicationCommandInput<'listAgentConfigs'>>().toEqualTypeOf<undefined>()
    expectTypeOf<ApplicationCommandResult<'listAgentConfigs'>>().toEqualTypeOf<AgentConfig[]>()
    expectTypeOf<ApplicationCommandInput<'saveAgentConfig'>>().toEqualTypeOf<SaveAgentConfigInput>()
    expectTypeOf<ApplicationCommandResult<'saveAgentConfig'>>().toEqualTypeOf<AgentConfig>()
    expectTypeOf<ApplicationCommandInput<'deleteAgentConfig'>>().toEqualTypeOf<AgentConfigInput>()
    expectTypeOf<ApplicationCommandResult<'deleteAgentConfig'>>().toEqualTypeOf<void>()
    expectTypeOf<ApplicationCommandResult<'checkAgentConfig'>>().toEqualTypeOf<AgentConnectionCheck>()
    expectTypeOf<ApplicationCommandInput<'createManagedSession'>>().toEqualTypeOf<CreateManagedSessionInput>()
    expectTypeOf<ApplicationCommandInput<'getManagedSession'>>().toEqualTypeOf<ManagedSessionInput>()
    expectTypeOf<ApplicationCommandInput<'sendManagedPrompt'>>().toEqualTypeOf<SendManagedPromptInput>()
    expectTypeOf<ApplicationCommandInput<'respondManagedPermission'>>().toEqualTypeOf<RespondManagedPermissionInput>()
    expectTypeOf<ApplicationCommandInput<'resolveFeedbackDelivery'>>().toEqualTypeOf<ResolveFeedbackDeliveryInput>()
    expectTypeOf<ApplicationCommandResult<'resolveFeedbackDelivery'>>().toEqualTypeOf<ManagedSessionSnapshot>()
    expectTypeOf<ApplicationCommandInput<'deleteManagedSession'>>().toEqualTypeOf<ManagedSessionInput>()
    expectTypeOf<ApplicationCommandResult<'deleteManagedSession'>>().toEqualTypeOf<void>()
    expectTypeOf<ApplicationCommandResult<'getManagedSession'>>().toEqualTypeOf<ManagedSessionSnapshot>()
    expectTypeOf<ApplicationCommandInput<'listFeedbackInbox'>>().toEqualTypeOf<undefined>()
    expectTypeOf<ApplicationCommandResult<'listFeedbackInbox'>>().toEqualTypeOf<
      FeedbackRequestSummary[]
    >()
    expectTypeOf<ApplicationCommandInput<'listHostProfiles'>>().toEqualTypeOf<undefined>()
    expectTypeOf<ApplicationCommandResult<'listHostProfiles'>>().toEqualTypeOf<
      ApplicationHostProfileView[]
    >()
    expectTypeOf<ApplicationCommandInput<'listFeedbackRequests'>>().toEqualTypeOf<
      ListFeedbackRequestsInput
    >()
    expectTypeOf<ApplicationCommandInput<'getFeedbackWorkspace'>>().toEqualTypeOf<
      GetFeedbackInput
    >()
    expectTypeOf<ApplicationCommandResult<'getFeedbackWorkspace'>>().toEqualTypeOf<
      ApplicationFeedbackWorkspaceView
    >()
    type HasResultStoragePaths = 'markdown_path' extends keyof NonNullable<
      ApplicationFeedbackWorkspaceView['feedback']
    >
      ? true
      : false
    expectTypeOf<HasResultStoragePaths>().toEqualTypeOf<false>()
    expectTypeOf<ApplicationCommandResult<'readPublishedFeedback'>>().toEqualTypeOf<
      FeedbackPackageView | null
    >()
    expectTypeOf<FeedbackPackageView['manifest']['source_revision']>().toEqualTypeOf<number>()
    expectTypeOf<FeedbackPackageView['manifest']['draft_revision']>().toEqualTypeOf<number>()
    expectTypeOf<
      FeedbackPackageView['manifest']['attachments'][number]['byte_size']
    >().toEqualTypeOf<number>()
    type HasStoragePaths = 'attachment_paths' extends keyof FeedbackPackageView ? true : false
    expectTypeOf<HasStoragePaths>().toEqualTypeOf<false>()
    expectTypeOf<ApplicationCommandResult<'saveFeedbackDraft'>>().toEqualTypeOf<DraftView>()
    expectTypeOf<ApplicationCommandInput<'addFeedbackAttachment'>>().toEqualTypeOf<
      ApplicationAddAttachmentInput
    >()
    expectTypeOf<ApplicationAddAttachmentInput['contents']>().toEqualTypeOf<ArrayBuffer>()
    expectTypeOf<ApplicationCommandInput<'readFeedbackAttachment'>>().toEqualTypeOf<
      ReadAttachmentInput
    >()
    expectTypeOf<ApplicationCommandResult<'readFeedbackAttachment'>>().toEqualTypeOf<ArrayBuffer>()
  })

  it('contains only the intended cross-client operation names', () => {
    const commands = [
      'setManagedSessionConfig',
      'listAvailableAgents', 'inspectAgentInstallation', 'listAgentInstallJobs', 'installAgent', 'cancelAgentInstall',
      'listAgentConfigs',
      'saveAgentConfig',
      'deleteAgentConfig',
      'checkAgentConfig',
      'createManagedSession',
      'getManagedSession',
      'startManagedSession',
      'stopManagedSession',
      'cancelManagedPrompt',
      'sendManagedPrompt',
      'respondManagedPermission',
      'resolveFeedbackDelivery',
      'deleteManagedSession',
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

    expect(commands).toHaveLength(42)
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
