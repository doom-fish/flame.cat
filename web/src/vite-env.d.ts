/// <reference types="vite/client" />

declare module "*.wgsl?raw" {
  const content: string;
  export default content;
}

declare module "*/flame_cat_wasm.js" {
  export default function init(): Promise<void>;
  export function parse_profile(data: Uint8Array): number;
  export function render_view(
    profile_index: number,
    view_type: string,
    x: number,
    y: number,
    width: number,
    height: number,
    dpr: number,
    selected_frame_id: number | undefined,
  ): string;
  export function get_profile_metadata(profile_index: number): string;
  export function get_frame_count(profile_index: number): number;
}
