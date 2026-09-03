// Tiny Web Audio sound engine — all effects are synthesized oscillators, no
// assets required. Audio starts on the first user gesture to satisfy autoplay
// policies.

export class Audio {
  private ctx: AudioContext | null = null;
  private muted = false;
  private waka = false;

  /** Create/resume the context. Call from a user gesture (keydown/click). */
  ensure(): AudioContext | null {
    if (this.muted) return this.ctx;
    if (!this.ctx) {
      const AC =
        window.AudioContext ||
        (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!AC) return null;
      this.ctx = new AC();
    }
    if (this.ctx.state === 'suspended') {
      void this.ctx.resume();
    }
    return this.ctx;
  }

  toggleMute(): boolean {
    this.muted = !this.muted;
    return this.muted;
  }

  isMuted(): boolean {
    return this.muted;
  }

  private beep(
    freq: number,
    duration: number,
    type: OscillatorType = 'square',
    volume = 0.04,
    sweepTo?: number,
  ): void {
    if (this.muted) return;
    const ctx = this.ensure();
    if (!ctx) return;
    const t = ctx.currentTime;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t);
    if (sweepTo) osc.frequency.exponentialRampToValueAtTime(sweepTo, t + duration);
    gain.gain.setValueAtTime(volume, t);
    gain.gain.exponentialRampToValueAtTime(0.0001, t + duration);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(t);
    osc.stop(t + duration);
  }

  /** Classic alternating "waka waka" dot munch. */
  dot(): void {
    this.waka = !this.waka;
    this.beep(this.waka ? 520 : 360, 0.07, 'square', 0.035);
  }

  pellet(): void {
    this.beep(720, 0.16, 'square', 0.05, 1400);
  }

  ghost(): void {
    this.beep(220, 0.3, 'sawtooth', 0.06, 700);
  }

  death(): void {
    this.beep(440, 0.9, 'sawtooth', 0.07, 60);
  }

  level(): void {
    this.beep(523, 0.18, 'triangle', 0.06, 1046);
    setTimeout(() => this.beep(1046, 0.18, 'triangle', 0.06), 120);
  }

  fright(): void {
    this.beep(680, 0.35, 'triangle', 0.05, 280);
  }
}
