export function imageAttachments(content) {
  if (!Array.isArray(content)) return [];
  return content.flatMap((part) => {
    if (!part || typeof part !== "object") return [];
    const mime = part.mime ?? part.mimeType;
    if (
      !mime?.startsWith("image/") &&
      !["image", "localImage", "inputImage"].includes(part.type)
    )
      return [];
    const src =
      part.uri ??
      part.url ??
      part.imageUrl ??
      part.path ??
      (part.data ? `data:${mime ?? "image/png"};base64,${part.data}` : null);
    return typeof src === "string" && src
      ? [{ src, alt: part.name ?? part.description ?? "Image" }]
      : [];
  });
}

export function toolImages(value) {
  if (Array.isArray(value)) return imageAttachments(value);
  return imageAttachments(value?.content);
}

export function mediaHash(src) {
  return /^media:([0-9a-f]{64})$/.exec(src)?.[1] ?? null;
}

export function inlineImage(src) {
  return (
    /^data:image\/(?:png|jpeg|gif|webp|avif|bmp|x-icon|svg\+xml);base64,[a-z\d+/=\s]+$/i.test(
      src,
    ) && src.length <= 14 * 1024 * 1024
  );
}
