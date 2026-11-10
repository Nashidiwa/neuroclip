/**
 * Hardware Profile Data Contract & IPC Service Wrapper
 * Conforms 100% to docs/ARCHITECTURE.md §5.1 (Schema hardware_profile.json).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { invoke } from '@tauri-apps/api/core';
import { parseIpcError } from '../ipc/error.ts';

// ---------------------------------------------------------------------------
// Canonical Enums & Primitives
// ---------------------------------------------------------------------------

export type GpuVendor = 'NVIDIA' | 'AMD' | 'Intel' | 'Unknown';
export type RecommendedMode = 'cloud_byok' | 'hybrid' | 'offline_local';

// ---------------------------------------------------------------------------
// Raw Wire Format (snake_case - Exact Schema 5.1 Representation)
// ---------------------------------------------------------------------------

export interface CpuInfoJson {
  readonly model: string;
  readonly cores: number;
  readonly threads: number;
}

export interface RamInfoJson {
  readonly total_mb: number;
  readonly available_mb: number;
}

export interface GpuInfoJson {
  readonly vendor: GpuVendor;
  readonly model: string;
  readonly vram_mb: number;
  readonly cuda_supported: boolean;
}

export interface HardwareProfileJson {
  readonly cpu_info: CpuInfoJson;
  readonly ram_info: RamInfoJson;
  readonly gpu_info: GpuInfoJson;
  readonly recommended_mode: RecommendedMode;
  readonly recommendation_reason?: string | undefined;
  readonly timestamp: string;
}

// ---------------------------------------------------------------------------
// Frontend Domain Models (camelCase)
// ---------------------------------------------------------------------------

export interface CpuInfo {
  readonly model: string;
  readonly cores: number;
  readonly threads: number;
}

export interface RamInfo {
  readonly totalMb: number;
  readonly availableMb: number;
}

export interface GpuInfo {
  readonly vendor: GpuVendor;
  readonly model: string;
  readonly vramMb: number;
  readonly cudaSupported: boolean;
}

export interface HardwareProfile {
  readonly cpuInfo: CpuInfo;
  readonly ramInfo: RamInfo;
  readonly gpuInfo: GpuInfo;
  readonly recommendedMode: RecommendedMode;
  readonly recommendationReason?: string | undefined;
  readonly timestamp: string;
}

// ---------------------------------------------------------------------------
// Zero-any Runtime Type Guards & Validators
// ---------------------------------------------------------------------------

export function isGpuVendor(val: unknown): val is GpuVendor {
  return val === 'NVIDIA' || val === 'AMD' || val === 'Intel' || val === 'Unknown';
}

export function isRecommendedMode(val: unknown): val is RecommendedMode {
  return val === 'cloud_byok' || val === 'hybrid' || val === 'offline_local';
}

export function isCpuInfoJson(val: unknown): val is CpuInfoJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['model'] === 'string' &&
    typeof obj['cores'] === 'number' &&
    Number.isInteger(obj['cores']) &&
    obj['cores'] > 0 &&
    typeof obj['threads'] === 'number' &&
    Number.isInteger(obj['threads']) &&
    obj['threads'] > 0
  );
}

export function isRamInfoJson(val: unknown): val is RamInfoJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['total_mb'] === 'number' &&
    Number.isInteger(obj['total_mb']) &&
    obj['total_mb'] >= 0 &&
    typeof obj['available_mb'] === 'number' &&
    Number.isInteger(obj['available_mb']) &&
    obj['available_mb'] >= 0
  );
}

export function isGpuInfoJson(val: unknown): val is GpuInfoJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    isGpuVendor(obj['vendor']) &&
    typeof obj['model'] === 'string' &&
    typeof obj['vram_mb'] === 'number' &&
    Number.isInteger(obj['vram_mb']) &&
    obj['vram_mb'] >= 0 &&
    typeof obj['cuda_supported'] === 'boolean'
  );
}

export function isHardwareProfileJson(val: unknown): val is HardwareProfileJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  const hasReason =
    obj['recommendation_reason'] === undefined ||
    typeof obj['recommendation_reason'] === 'string';

  return (
    isCpuInfoJson(obj['cpu_info']) &&
    isRamInfoJson(obj['ram_info']) &&
    isGpuInfoJson(obj['gpu_info']) &&
    isRecommendedMode(obj['recommended_mode']) &&
    hasReason &&
    typeof obj['timestamp'] === 'string'
  );
}

export function isHardwareProfile(val: unknown): val is HardwareProfile {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  const cpu = obj['cpuInfo'];
  const ram = obj['ramInfo'];
  const gpu = obj['gpuInfo'];

  if (typeof cpu !== 'object' || cpu === null) return false;
  if (typeof ram !== 'object' || ram === null) return false;
  if (typeof gpu !== 'object' || gpu === null) return false;

  const cpuObj = cpu as Record<string, unknown>;
  const ramObj = ram as Record<string, unknown>;
  const gpuObj = gpu as Record<string, unknown>;

  const hasReason =
    obj['recommendationReason'] === undefined ||
    typeof obj['recommendationReason'] === 'string';

  return (
    typeof cpuObj['model'] === 'string' &&
    typeof cpuObj['cores'] === 'number' &&
    typeof cpuObj['threads'] === 'number' &&
    typeof ramObj['totalMb'] === 'number' &&
    typeof ramObj['availableMb'] === 'number' &&
    isGpuVendor(gpuObj['vendor']) &&
    typeof gpuObj['model'] === 'string' &&
    typeof gpuObj['vramMb'] === 'number' &&
    typeof gpuObj['cudaSupported'] === 'boolean' &&
    isRecommendedMode(obj['recommendedMode']) &&
    hasReason &&
    typeof obj['timestamp'] === 'string'
  );
}

// ---------------------------------------------------------------------------
// Lossless Bidirectional Serialization Mappers
// ---------------------------------------------------------------------------

export function toHardwareProfile(
  raw: HardwareProfileJson | HardwareProfile,
): HardwareProfile {
  if ('cpuInfo' in raw) {
    return raw;
  }

  const profile: HardwareProfile = {
    cpuInfo: {
      model: raw.cpu_info.model,
      cores: raw.cpu_info.cores,
      threads: raw.cpu_info.threads,
    },
    ramInfo: {
      totalMb: raw.ram_info.total_mb,
      availableMb: raw.ram_info.available_mb,
    },
    gpuInfo: {
      vendor: raw.gpu_info.vendor,
      model: raw.gpu_info.model,
      vramMb: raw.gpu_info.vram_mb,
      cudaSupported: raw.gpu_info.cuda_supported,
    },
    recommendedMode: raw.recommended_mode,
    timestamp: raw.timestamp,
    ...(typeof raw.recommendation_reason === 'string' && raw.recommendation_reason.length > 0
      ? { recommendationReason: raw.recommendation_reason }
      : {}),
  };

  return profile;
}

export function toHardwareProfileJson(profile: HardwareProfile): HardwareProfileJson {
  const json: HardwareProfileJson = {
    cpu_info: {
      model: profile.cpuInfo.model,
      cores: profile.cpuInfo.cores,
      threads: profile.cpuInfo.threads,
    },
    ram_info: {
      total_mb: profile.ramInfo.totalMb,
      available_mb: profile.ramInfo.availableMb,
    },
    gpu_info: {
      vendor: profile.gpuInfo.vendor,
      model: profile.gpuInfo.model,
      vram_mb: profile.gpuInfo.vramMb,
      cuda_supported: profile.gpuInfo.cudaSupported,
    },
    recommended_mode: profile.recommendedMode,
    timestamp: profile.timestamp,
    ...(typeof profile.recommendationReason === 'string' &&
    profile.recommendationReason.length > 0
      ? { recommendation_reason: profile.recommendationReason }
      : {}),
  };

  return json;
}

// ---------------------------------------------------------------------------
// IPC Invoke Wrappers
// ---------------------------------------------------------------------------

/**
 * Retrieves the cached hardware profile if available.
 */
export async function fetchHardwareProfile(): Promise<HardwareProfile | null> {
  try {
    const raw = await invoke<HardwareProfileJson | null>('get_hardware_profile');
    if (raw === null || raw === undefined) {
      return null;
    }
    return toHardwareProfile(raw);
  } catch (err: unknown) {
    const parsed = parseIpcError(err, 'Không thể tải thông số phần cứng');
    throw parsed;
  }
}

/**
 * Triggers a live hardware probe in the backend and returns the evaluated profile.
 */
export async function detectHardwareProfile(): Promise<HardwareProfile> {
  try {
    const raw = await invoke<HardwareProfileJson>('detect_hardware');
    return toHardwareProfile(raw);
  } catch (err: unknown) {
    const parsed = parseIpcError(err, 'Không thể đánh giá cấu hình phần cứng');
    throw parsed;
  }
}
