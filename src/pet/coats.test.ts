import { describe, expect, it } from "vitest";
import { COATS, coatById } from "./coats";

const HEX = /^#[0-9a-fA-F]{6}$/;

describe("built-in coats", () => {
  it("has unique ids", () => {
    expect(new Set(COATS.map((c) => c.id)).size).toBe(COATS.length);
  });

  it("uses valid hex colours for the slots the UI previews", () => {
    for (const coat of COATS) {
      for (const key of ["fur", "paw", "inner", "eye", "whisker", "line"] as const) {
        expect(coat[key], `${coat.id}.${key}`).toMatch(HEX);
      }
    }
  });

  it("only uses patterns the settings UI can express", () => {
    // The renderer additionally knows the built-in "point" pattern; custom
    // cats only support these four.
    for (const coat of COATS) {
      expect(["solid", "tabby", "calico", "point"]).toContain(coat.pattern);
    }
  });

  it("falls back to the first coat for unknown ids", () => {
    expect(coatById("nope").id).toBe(COATS[0].id);
  });
});
