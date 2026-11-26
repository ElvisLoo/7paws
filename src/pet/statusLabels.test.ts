import { describe, expect, it } from "vitest";
import { describePetState } from "./statusLabels";

describe("describePetState", () => {
  it("labels every action in the renderer pool", () => {
    const actions = ["lookL", "lookR", "tilt", "ear", "lick", "groom", "loaf", "curl", "stretch", "yawn", "tail"];
    for (const action of actions) {
      expect(describePetState("idle", action)).toBeTruthy();
    }
  });

  it("gives each idle action a distinct label", () => {
    const labels = new Set(["lick", "groom", "loaf", "yawn"].map((a) => describePetState("idle", a)));
    expect(labels.size).toBe(4);
  });

  it("shows the state while an action plays on top of typing or mouse play", () => {
    expect(describePetState("typing", "stretch")).toBe("伸个懒腰");
    expect(describePetState("mouse", "lick")).toBe("舔舔爪子");
  });

  it("hides the bubble when idle with no action", () => {
    expect(describePetState("idle", "")).toBeNull();
  });

  it("lets sleeping win over a leftover action", () => {
    expect(describePetState("sleeping", "groom")).toContain("睡着");
  });

  it("falls back to interaction states without an action", () => {
    expect(describePetState("petting", "")).toContain("摸摸");
    expect(describePetState("typing", "")).toContain("打字");
    expect(describePetState("mouse", "")).toContain("鼠标");
    expect(describePetState("sleeping", "")).toContain("睡着");
  });

  it("ignores unknown actions and unknown states", () => {
    expect(describePetState("idle", "nope")).toBeNull();
    expect(describePetState("dancing", "")).toBeNull();
    expect(describePetState("dancing", "nope")).toBeNull();
  });
});
