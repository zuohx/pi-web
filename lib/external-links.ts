"use client";

type AnchorClickEvent = Pick<MouseEvent, "button" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey" | "preventDefault">;

function isModifiedEvent(event: AnchorClickEvent): boolean {
  return event.metaKey || event.ctrlKey || event.shiftKey || event.altKey;
}

function isProbablyExternal(url: string): boolean {
  return /^(https?:)?\/\//i.test(url);
}

export async function openExternalUrl(url: string): Promise<void> {
  if (typeof window === "undefined") return;

  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
    return;
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

export async function handleExternalLinkClick(
  event: AnchorClickEvent,
  href: string | undefined | null,
): Promise<void> {
  if (!href || typeof window === "undefined") return;
  if (!isProbablyExternal(href)) return;
  if (event.button !== 0 || isModifiedEvent(event)) return;

  event.preventDefault();
  await openExternalUrl(href);
}
