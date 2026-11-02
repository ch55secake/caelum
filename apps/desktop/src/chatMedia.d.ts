export type ChatImageSource = { src: string; alt: string };
export function imageAttachments(content: unknown): ChatImageSource[];
export function toolImages(value: unknown): ChatImageSource[];
export function mediaHash(src: string): string | null;
export function inlineImage(src: string): boolean;
