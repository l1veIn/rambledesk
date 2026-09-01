export class StreamingResampler {
  #buffer = new Float32Array(0)
  #bufferStart = 0
  #totalInput = 0
  #nextOutput = 0

  constructor(readonly inputRate: number, readonly outputRate = 16_000) {
    if (!(inputRate > 0) || !(outputRate > 0)) throw new RangeError('Sample rates must be positive.')
  }

  push(input: Float32Array): Float32Array {
    if (input.length === 0) return new Float32Array(0)
    const data = new Float32Array(this.#buffer.length + input.length)
    data.set(this.#buffer)
    data.set(input, this.#buffer.length)
    this.#totalInput += input.length
    const output: number[] = []
    let position = this.#sourcePosition()
    while (position + 1 < this.#totalInput) {
      const absoluteIndex = Math.floor(position)
      const index = absoluteIndex - this.#bufferStart
      const fraction = position - absoluteIndex
      output.push(data[index] + (data[index + 1] - data[index]) * fraction)
      this.#nextOutput += 1
      position = this.#sourcePosition()
    }
    const nextRequired = Math.min(Math.floor(position), this.#totalInput - 1)
    const consumed = Math.max(0, nextRequired - this.#bufferStart)
    this.#buffer = data.slice(consumed)
    this.#bufferStart += consumed
    return Float32Array.from(output)
  }

  flush(): Float32Array {
    if (this.#buffer.length === 0) return new Float32Array(0)
    const output: number[] = []
    let position = this.#sourcePosition()
    while (position < this.#totalInput) {
      const index = Math.floor(position) - this.#bufferStart
      const next = Math.min(index + 1, this.#buffer.length - 1)
      const fraction = position - Math.floor(position)
      output.push(this.#buffer[index] + (this.#buffer[next] - this.#buffer[index]) * fraction)
      this.#nextOutput += 1
      position = this.#sourcePosition()
    }
    this.#buffer = new Float32Array(0)
    this.#bufferStart = 0
    this.#totalInput = 0
    this.#nextOutput = 0
    return Float32Array.from(output)
  }

  #sourcePosition(): number {
    return this.#nextOutput * this.inputRate / this.outputRate
  }
}

export function downmixToMono(channels: readonly Float32Array[]): Float32Array {
  if (channels.length === 0) return new Float32Array(0)
  if (channels.length === 1) return channels[0].slice()
  const length = Math.min(...channels.map((channel) => channel.length))
  const mono = new Float32Array(length)
  for (const channel of channels) {
    for (let index = 0; index < length; index += 1) mono[index] += channel[index] / channels.length
  }
  return mono
}
