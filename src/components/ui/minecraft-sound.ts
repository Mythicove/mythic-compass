import { minecraftClickSound } from "./minecraft-sound-assets";

let masterVolume = 1;

/** Master volume from 0 (muted) to 1; native per-event gain stays unchanged. */
export const setMinecraftSoundVolume = function setMinecraftSoundVolume(
  volume: number
) {
  if (Number.isFinite(volume)) {
    masterVolume = Math.min(1, Math.max(0, volume));
  }
};

export const getMinecraftSoundVolume = function getMinecraftSoundVolume() {
  return masterVolume;
};

/** Original ui.button.click, with SimpleSoundInstance.forUI's pitch and gain. */
export const playMinecraftClick = function playMinecraftClick() {
  if (typeof Audio === "undefined" || masterVolume === 0) {
    return;
  }

  const sound = new Audio(minecraftClickSound);
  sound.volume = 0.25 * masterVolume;
  sound.playbackRate = 1;
  // A separate voice allows successive activations to overlap as in the game.
  void sound.play().catch(() => {
    /* empty */
  });
};
