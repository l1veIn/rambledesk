class RamblePcmCaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super()
    this.pcmPort = null
    this.sessionId = ''
    this.credits = 0
    this.seq = 0
    this.droppedFrames = 0
    this.batch = []
    this.batchLength = 0
    this.targetFrames = Math.max(128, Math.round(sampleRate * 0.05))
    this.lastLevelFrame = -sampleRate
    this.port.onmessage = ({ data }) => {
      if (data?.type === 'bind' && data.port) {
        this.sessionId = data.sessionId
        this.credits = data.credits
        this.pcmPort = data.port
        this.pcmPort.onmessage = ({ data: ack }) => {
          if (ack?.v === 1 && ack.type === 'ack' && ack.sessionId === this.sessionId) {
            this.credits += ack.credit
          }
        }
        this.pcmPort.start()
      } else if (data?.type === 'flush') {
        this.flush()
        this.port.postMessage({ type: 'flushed', lastSeq: this.seq, droppedFrames: this.droppedFrames })
      }
    }
  }

  process(inputs) {
    const channels = inputs[0]
    if (!this.pcmPort || !channels || channels.length === 0) return true
    const length = channels[0].length
    const mono = new Float32Array(length)
    let energy = 0
    for (let index = 0; index < length; index += 1) {
      let sample = 0
      for (const channel of channels) sample += channel[index] / channels.length
      mono[index] = sample
      energy += sample * sample
    }
    if (currentFrame - this.lastLevelFrame >= sampleRate / 10) {
      this.lastLevelFrame = currentFrame
      this.port.postMessage({ type: 'level', rms: Math.sqrt(energy / Math.max(1, length)) })
    }
    this.batch.push(mono)
    this.batchLength += mono.length
    if (this.batchLength >= this.targetFrames) this.flush()
    return true
  }

  flush() {
    if (this.batchLength === 0 || !this.pcmPort) return
    const samples = new Float32Array(this.batchLength)
    let offset = 0
    for (const chunk of this.batch) {
      samples.set(chunk, offset)
      offset += chunk.length
    }
    this.batch = []
    this.batchLength = 0
    if (this.credits <= 0) {
      this.droppedFrames += samples.length
      return
    }
    this.credits -= 1
    this.seq += 1
    this.pcmPort.postMessage({
      v: 1,
      type: 'pcm',
      sessionId: this.sessionId,
      seq: this.seq,
      sampleRate,
      samples: samples.buffer,
    }, [samples.buffer])
  }
}

registerProcessor('rambledesk-pcm-capture', RamblePcmCaptureProcessor)
