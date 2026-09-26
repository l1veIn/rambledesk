import { getContext, setContext } from 'svelte'
import type { RequestSpeechTidyController } from './requestSpeechTidy'

const REQUEST_SPEECH_TOOLS = Symbol('rambledesk.request-speech-tools')
export function provideRequestSpeechTools(controller: RequestSpeechTidyController) { setContext(REQUEST_SPEECH_TOOLS, controller) }
export function useRequestSpeechTools(): RequestSpeechTidyController | undefined { return getContext(REQUEST_SPEECH_TOOLS) }
