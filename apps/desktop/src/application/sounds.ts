let context: AudioContext | undefined;
let idle: ReturnType<typeof setTimeout> | undefined;

export async function prepareSound() {
  context ??= new AudioContext();
  if (context.state === "suspended") {
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([
        context.resume(),
        new Promise<never>((_, reject) => {
          timeout = setTimeout(
            () =>
              reject(
                new Error(
                  "Sound playback is unavailable. Try Test sound again.",
                ),
              ),
            500,
          );
        }),
      ]);
    } finally {
      clearTimeout(timeout);
    }
  }
  clearTimeout(idle);
  idle = setTimeout(() => {
    void context?.suspend().catch(() => {});
  }, 500);
  return context;
}

// Short synthesized tones avoid loading media or spawning a process. Nodes are
// disconnected after playback; the context is shared across all sessions.
export async function playSound(volume: number, attention = false) {
  if (volume <= 0) return;
  const audio = await prepareSound();
  if (audio.state !== "running")
    throw new Error("Sound playback is unavailable. Try Test sound again.");
  const start = audio.currentTime;
  const gain = audio.createGain();
  gain.connect(audio.destination);
  const loudness = Math.min(1, volume / 100) ** 2 * 0.16;
  gain.gain.setValueAtTime(0, start);
  gain.gain.linearRampToValueAtTime(loudness, start + 0.015);
  gain.gain.exponentialRampToValueAtTime(
    Math.min(0.0001, loudness / 10),
    start + 0.32,
  );
  const oscillator = audio.createOscillator();
  oscillator.type = "sine";
  oscillator.frequency.setValueAtTime(attention ? 660 : 880, start);
  oscillator.frequency.setValueAtTime(attention ? 880 : 1174.66, start + 0.13);
  oscillator.connect(gain);
  oscillator.onended = () => {
    oscillator.disconnect();
    gain.disconnect();
  };
  oscillator.start(start);
  oscillator.stop(start + 0.34);
}
