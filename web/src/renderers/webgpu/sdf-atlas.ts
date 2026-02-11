/**
 * SDF (Signed Distance Field) glyph atlas generator.
 * Renders ASCII glyphs to an offscreen canvas, computes SDF, and uploads as a GPU texture.
 */

export interface GlyphMetrics {
  /** UV coordinates in atlas (0-1) */
  u0: number; v0: number; u1: number; v1: number;
  /** Glyph dimensions in pixels at render size */
  width: number; height: number;
  /** Horizontal advance */
  advance: number;
  /** Bearing (offset from baseline) */
  bearingX: number; bearingY: number;
}

export interface GlyphAtlas {
  texture: GPUTexture;
  metrics: Map<string, GlyphMetrics>;
  lineHeight: number;
  /** The font size the atlas was rendered at */
  atlasSize: number;
}

const ATLAS_DIM = 512;
const RENDER_SIZE = 48; // px - high-res for SDF quality
const SDF_SPREAD = 6;  // pixel spread for distance field
const PADDING = 2;     // padding between glyphs in atlas

/**
 * Generate SDF atlas covering ASCII 32-126 + ellipsis
 */
export function generateSDFAtlas(device: GPUDevice): GlyphAtlas {
  const chars: string[] = [];
  for (let i = 32; i <= 126; i++) chars.push(String.fromCharCode(i));
  chars.push("…"); // ellipsis for truncation

  // Step 1: Render glyphs to offscreen canvas and measure
  const measure = document.createElement("canvas");
  const mctx = measure.getContext("2d");
  if (!mctx) throw new Error("Cannot create 2D context for SDF atlas");
  mctx.font = `${RENDER_SIZE}px sans-serif`;

  // Measure all glyphs first
  const glyphInfo: { char: string; width: number; cellW: number }[] = [];
  const cellH = RENDER_SIZE + SDF_SPREAD * 2;
  let maxCellW = 0;

  for (const ch of chars) {
    const tm = mctx.measureText(ch);
    const cellW = Math.ceil(tm.width) + SDF_SPREAD * 2;
    glyphInfo.push({ char: ch, width: tm.width, cellW });
    maxCellW = Math.max(maxCellW, cellW);
  }

  // Render all glyphs on a single offscreen canvas
  const gc = document.createElement("canvas");
  gc.width = maxCellW;
  gc.height = cellH;
  const gctx = gc.getContext("2d");
  if (!gctx) throw new Error("Cannot create glyph render context");

  const glyphData: { char: string; width: number; imgData: ImageData }[] = [];

  for (const info of glyphInfo) {
    gctx.clearRect(0, 0, maxCellW, cellH);
    gctx.fillStyle = "#000";
    gctx.fillRect(0, 0, info.cellW, cellH);
    gctx.fillStyle = "#fff";
    gctx.font = `${RENDER_SIZE}px sans-serif`;
    gctx.textBaseline = "top";
    gctx.fillText(info.char, SDF_SPREAD, SDF_SPREAD);

    glyphData.push({
      char: info.char,
      width: info.width,
      imgData: gctx.getImageData(0, 0, info.cellW, cellH),
    });
  }

  // Step 2: Pack glyphs into atlas and compute SDF
  const atlasData = new Uint8Array(ATLAS_DIM * ATLAS_DIM);
  const metrics = new Map<string, GlyphMetrics>();

  let cursorX = 0;
  let cursorY = 0;
  let rowHeight = 0;

  for (const g of glyphData) {
    const gw = g.imgData.width;
    const gh = g.imgData.height;

    if (cursorX + gw > ATLAS_DIM) {
      cursorX = 0;
      cursorY += rowHeight + PADDING;
      rowHeight = 0;
    }
    if (cursorY + gh > ATLAS_DIM) {
      console.warn("SDF atlas overflow, skipping", g.char);
      continue;
    }

    // Compute SDF for this glyph
    const sdf = computeSDF(g.imgData, SDF_SPREAD);

    // Copy SDF data into atlas
    for (let y = 0; y < gh; y++) {
      for (let x = 0; x < gw; x++) {
        atlasData[(cursorY + y) * ATLAS_DIM + (cursorX + x)] = sdf[y * gw + x]!;
      }
    }

    metrics.set(g.char, {
      u0: cursorX / ATLAS_DIM,
      v0: cursorY / ATLAS_DIM,
      u1: (cursorX + gw) / ATLAS_DIM,
      v1: (cursorY + gh) / ATLAS_DIM,
      width: gw,
      height: gh,
      advance: g.width,
      bearingX: -SDF_SPREAD,
      bearingY: -SDF_SPREAD,
    });

    cursorX += gw + PADDING;
    rowHeight = Math.max(rowHeight, gh);
  }

  // Step 3: Upload to GPU
  const texture = device.createTexture({
    size: [ATLAS_DIM, ATLAS_DIM],
    format: "r8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });

  device.queue.writeTexture(
    { texture },
    atlasData,
    { bytesPerRow: ATLAS_DIM },
    [ATLAS_DIM, ATLAS_DIM],
  );

  return {
    texture,
    metrics,
    lineHeight: RENDER_SIZE,
    atlasSize: RENDER_SIZE,
  };
}

/**
 * Compute SDF from a binary glyph image using brute-force distance transform.
 * Returns Uint8Array where 128 = on boundary, >128 = inside, <128 = outside.
 */
function computeSDF(imgData: ImageData, spread: number): Uint8Array {
  const w = imgData.width;
  const h = imgData.height;
  const pixels = imgData.data;
  const result = new Uint8Array(w * h);

  // Extract binary bitmap (threshold at 128)
  const inside = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) {
    inside[i] = pixels[i * 4]! > 128 ? 1 : 0;
  }

  // For each pixel, find distance to nearest opposite pixel
  // Use a bounded search window for performance
  const searchRadius = spread + 1;

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const idx = y * w + x;
      const isInside = inside[idx];
      let minDistSq = spread * spread * 4; // large initial value

      const x0 = Math.max(0, x - searchRadius);
      const x1 = Math.min(w - 1, x + searchRadius);
      const y0 = Math.max(0, y - searchRadius);
      const y1 = Math.min(h - 1, y + searchRadius);

      for (let sy = y0; sy <= y1; sy++) {
        for (let sx = x0; sx <= x1; sx++) {
          if (inside[sy * w + sx] !== isInside) {
            const dx = x - sx;
            const dy = y - sy;
            const dSq = dx * dx + dy * dy;
            if (dSq < minDistSq) minDistSq = dSq;
          }
        }
      }

      const dist = Math.sqrt(minDistSq);
      // Normalize: inside = positive, outside = negative
      const signedDist = isInside ? dist : -dist;
      // Map to 0-255 range: 128 = boundary, 255 = deep inside, 0 = far outside
      const normalized = signedDist / spread * 0.5 + 0.5;
      result[idx] = Math.max(0, Math.min(255, Math.round(normalized * 255)));
    }
  }

  return result;
}
