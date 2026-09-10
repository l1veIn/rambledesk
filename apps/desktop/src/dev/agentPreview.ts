import '../app.css'
import { mount } from 'svelte'
import AgentPreview from './AgentPreview.svelte'
if (import.meta.env.DEV) mount(AgentPreview, { target: document.getElementById('app')! })
