import type { AppSettings } from "./types";

export const CAPTURE_BACKGROUNDS: {
  value: AppSettings["captureBackground"];
  label: string;
  color: string;
}[] = [
  { value: "green", label: "绿幕", color: "#00ff00" },
  { value: "blue", label: "蓝幕", color: "#0000ff" },
  { value: "magenta", label: "品红幕", color: "#ff00ff" },
];
