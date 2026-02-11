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
    selected_frame_id?: bigint | null,
  ): string;
  export function get_profile_metadata(profile_index: number): string;
  export function get_frame_count(profile_index: number): number;
  export function render_minimap(
    profile_index: number,
    width: number,
    height: number,
    dpr: number,
    visible_start_frac: number,
    visible_end_frac: number,
  ): string;
  export function get_ranked_entries(
    profile_index: number,
    sort_field: string,
    ascending: boolean,
  ): string;
}
