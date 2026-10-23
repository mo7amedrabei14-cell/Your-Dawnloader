import { browser } from "wxt/browser";

type Candidate = { url: string; title: string; kind: string };
const scanButton = document.querySelector<HTMLButtonElement>("#scan")!;
const message = document.querySelector<HTMLElement>("#message")!;
const results = document.querySelector<HTMLElement>("#results")!;

function safeFileName(rawUrl: string): string {
  try {
    const url = new URL(rawUrl);
    const raw = url.pathname.split("/").filter(Boolean).pop() || "media-download";
    return raw.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_").slice(0, 150);
  } catch {
    return "media-download";
  }
}

function renderCandidates(candidates: Candidate[]) {
  results.replaceChildren();
  if (candidates.length === 0) {
    message.textContent = "No direct media candidates found. Blob/MSE, protected streams, and provider pages may not expose a direct file.";
    return;
  }
  message.textContent = "Found " + candidates.length + " candidate(s). Select a file to send it to the browser download manager.";
  for (const candidate of candidates) {
    const card = document.createElement("article");
    card.className = "candidate";
    const title = document.createElement("strong");
    title.textContent = candidate.title || candidate.kind;
    const source = document.createElement("p");
    source.textContent = candidate.kind + " · " + new URL(candidate.url).hostname;
    const button = document.createElement("button");
    button.textContent = "Download this file";
    button.addEventListener("click", async () => {
      button.disabled = true;
      try {
        await browser.downloads.download({ url: candidate.url, filename: safeFileName(candidate.url), saveAs: true });
        button.textContent = "Sent to downloads";
      } catch {
        button.disabled = false;
        button.textContent = "Download unavailable";
      }
    });
    card.append(title, source, button);
    results.append(card);
  }
}

scanButton.addEventListener("click", async () => {
  results.replaceChildren();
  scanButton.disabled = true;
  message.textContent = "Inspecting the active tab…";
  try {
    const tabs = await browser.tabs.query({ active: true, lastFocusedWindow: true });
    const tab = tabs[0];
    if (!tab?.id || !tab.url || !/^https?:/i.test(tab.url)) {
      message.textContent = "Open a regular website first. Browser-internal pages cannot be scanned.";
      return;
    }
    const injected = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: (): { candidates: Candidate[]; pageTitle: string } => {
        const knownExtensions = /\.(mp3|m4a|aac|wav|ogg|opus|flac|mp4|m4v|mov|webm|mkv|avi)(?:$|[?#])/i;
        const found = new Map<string, Candidate>();
        const add = (raw: string | null | undefined, title: string, kind: string) => {
          if (!raw) return;
          try {
            const url = new URL(raw, location.href);
            if (!["http:", "https:"].includes(url.protocol)) return;
            if (!knownExtensions.test(url.href)) return;
            if (!found.has(url.href)) found.set(url.href, { url: url.href, title, kind });
          } catch { /* malformed page input is ignored */ }
        };
        document.querySelectorAll("audio,video").forEach((node, index) => {
          const media = node as HTMLMediaElement;
          add(media.currentSrc || media.getAttribute("src") || media.querySelector("source[src]")?.getAttribute("src"),
            media.getAttribute("aria-label") || document.title || ("Media " + (index + 1)),
            media instanceof HTMLAudioElement ? "audio" : "video");
        });
        document.querySelectorAll("a[href]").forEach((node) => {
          const link = node as HTMLAnchorElement;
          add(link.href, link.textContent?.trim() || document.title || "Media file", "direct link");
        });
        return { candidates: Array.from(found.values()), pageTitle: document.title };
      },
    });
    renderCandidates(injected[0]?.result?.candidates || []);
  } catch {
    message.textContent = "The browser did not allow scanning this tab. Try a regular HTTPS page.";
  } finally {
    scanButton.disabled = false;
  }
});
