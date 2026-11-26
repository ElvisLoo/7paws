// Maps renderer state()/idleAction() names onto the short Chinese labels the
// status bubble shows. Kept as a pure module so it can be unit-tested without a
// WebGL context; the renderer vocabulary lives in public/vendor/7paws.js.

export type PetState = "sleeping" | "petting" | "typing" | "mouse" | "idle";

const ACTION_LABELS: Record<string, string> = {
  lookL: "望向左边",
  lookR: "望向右边",
  tilt: "歪头好奇",
  ear: "抖动耳朵",
  lick: "舔舔爪子",
  groom: "梳理毛发",
  loaf: "揣手手",
  curl: "蜷起身体",
  stretch: "伸个懒腰",
  yawn: "打了个哈欠",
  tail: "甩甩尾巴",
};

const STATE_LABELS: Record<PetState, string> = {
  sleeping: "睡着了，呼噜呼噜",
  petting: "被摸摸，很享受",
  typing: "陪我一起打字",
  mouse: "在玩鼠标",
  idle: "发呆中",
};

/**
 * Resolve the label for a renderer tick. Returns null when nothing is worth
 * showing (idle with no action playing), which hides the bubble entirely.
 * Sleeping always wins: an action may still be animating as the cat dozes off.
 */
export function describePetState(state: string, action: string): string | null {
  if (state === "sleeping") return STATE_LABELS.sleeping;
  if (action && ACTION_LABELS[action]) return ACTION_LABELS[action];
  if (state === "petting" || state === "typing" || state === "mouse") {
    return STATE_LABELS[state];
  }
  return null;
}
