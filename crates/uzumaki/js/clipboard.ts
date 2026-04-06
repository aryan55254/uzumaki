import core from './core';

export const clipboard = {
  readText(): string | null {
    return core.readClipboardText();
  },
  writeText(text: string): boolean {
    return core.writeClipboardText(text);
  },
};
