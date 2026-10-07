export interface IDisposable {
  dispose(): void
}

/** Subscribe with `terminal.onData(listener)`; dispose the result to unsubscribe. */
export type IEvent<T> = (listener: (value: T) => void) => IDisposable

export class Emitter<T> {
  #listeners = new Set<(value: T) => void>()

  readonly event: IEvent<T> = (listener) => {
    this.#listeners.add(listener)
    return { dispose: () => this.#listeners.delete(listener) }
  }

  get hasListeners(): boolean {
    return this.#listeners.size > 0
  }

  fire(value: T): void {
    for (const listener of [...this.#listeners]) listener(value)
  }

  dispose(): void {
    this.#listeners.clear()
  }
}

export function toDisposable(dispose: () => void): IDisposable {
  return { dispose }
}
