import { describe, expect, it } from "vitest";
import { buildCoatPalette, luminance, mixHex } from "./catPalette";
import type { CustomCat } from "../types";

function cat(overrides: Partial<CustomCat> = {}): CustomCat {
  return {
    id: "my-cat",
    name: "我的猫",
    fur: "#e9973f",
    patch: "#fbe8c9",
    accent: "#c26a1f",
    eye: "#5c8a3a",
    eye2: "#5c8a3a",
    pattern: "bicolor",
    spots: [],
    ...overrides,
  };
}

describe("mixHex", () => {
  it("returns the endpoints at t=0 and t=1", () => {
    expect(mixHex("#000000", "#ffffff", 0)).toBe("#000000");
    expect(mixHex("#000000", "#ffffff", 1)).toBe("#ffffff");
  });

  it("interpolates channels independently", () => {
    expect(mixHex("#000000", "#ff0000", 0.5)).toBe("#800000");
  });
});

describe("luminance", () => {
  it("orders black below white", () => {
    expect(luminance("#000000")).toBeLessThan(luminance("#ffffff"));
  });
});

describe("buildCoatPalette", () => {
  it("derives outline/whisker colours from the coat instead of hardcoding", () => {
    const dark = buildCoatPalette(cat({ fur: "#101010" }));
    const pale = buildCoatPalette(cat({ fur: "#f8f6ef" }));
    expect(dark.line).not.toBe(pale.line);
    expect(dark.whisker).toBe("#f4f4ee");
    expect(pale.whisker).toBe("#b5b1a2");
  });

  it("solid cats ignore the patch colour", () => {
    const palette = buildCoatPalette(cat({ fur: "#101010", patch: "#ffffff", pattern: "solid" }));
    expect(palette.patch).toBe("#101010");
    expect(palette.stripe).toBe("#101010");
  });

  it("calico maps patch/accent onto patchA/patchB/tail", () => {
    const palette = buildCoatPalette(cat({ pattern: "calico" }));
    expect(palette.patchA).toBe("#fbe8c9");
    expect(palette.patchB).toBe("#c26a1f");
    expect(palette.tail).toBe("#c26a1f");
  });

  it("falls back to bicolor for patterns the renderer cannot draw", () => {
    const palette = buildCoatPalette(cat({ pattern: "point" as CustomCat["pattern"] }));
    expect(palette.pattern).toBe("bicolor");
  });

  it("defaults a missing eye2 to eye", () => {
    const palette = buildCoatPalette(cat({ eye2: "" }));
    expect(palette.eye2).toBe("#5c8a3a");
  });
});
