import { describe, it, expect } from 'vitest';
import Ajv from 'ajv';
import {
  type HardwareProfileJson,
  toHardwareProfile,
  toHardwareProfileJson,
  isHardwareProfileJson,
  isHardwareProfile,
  isGpuVendor,
  isRecommendedMode,
} from '../services/config/hardware';

export const HARDWARE_PROFILE_SCHEMA_5_1 = {
  $schema: 'http://json-schema.org/draft-07/schema#',
  title: 'HardwareProfile',
  type: 'object',
  required: ['cpu_info', 'ram_info', 'gpu_info', 'recommended_mode', 'timestamp'],
  properties: {
    cpu_info: {
      type: 'object',
      required: ['model', 'cores', 'threads'],
      properties: {
        model: { type: 'string' },
        cores: { type: 'integer' },
        threads: { type: 'integer' },
      },
    },
    ram_info: {
      type: 'object',
      required: ['total_mb', 'available_mb'],
      properties: {
        total_mb: { type: 'integer' },
        available_mb: { type: 'integer' },
      },
    },
    gpu_info: {
      type: 'object',
      required: ['vendor', 'model', 'vram_mb', 'cuda_supported'],
      properties: {
        vendor: { type: 'string', enum: ['NVIDIA', 'AMD', 'Intel', 'Unknown'] },
        model: { type: 'string' },
        vram_mb: { type: 'integer' },
        cuda_supported: { type: 'boolean' },
      },
    },
    recommended_mode: {
      type: 'string',
      enum: ['cloud_byok', 'hybrid', 'offline_local'],
    },
    recommendation_reason: { type: 'string' },
    timestamp: { type: 'string', format: 'date-time' },
  },
};

describe('TASK-M4-03: Hardware Profile JSON Schema 5.1 Compliance (UT-03.4)', () => {
  const ajv = new Ajv({ allErrors: true, format: 'full' });
  const validate = ajv.compile(HARDWARE_PROFILE_SCHEMA_5_1);

  it('should validate high-end, mid-range and low-end hardware profiles against Schema 5.1', () => {
    const highEndPayload: HardwareProfileJson = {
      cpu_info: { model: 'AMD Ryzen 9 7950X', cores: 16, threads: 32 },
      ram_info: { total_mb: 32768, available_mb: 24576 },
      gpu_info: {
        vendor: 'NVIDIA',
        model: 'GeForce RTX 4090',
        vram_mb: 24576,
        cuda_supported: true,
      },
      recommended_mode: 'offline_local',
      recommendation_reason: 'GPU 24GB VRAM và CUDA hỗ trợ tốt mô hình lớn',
      timestamp: '2026-11-09T20:00:00.000Z',
    };
    expect(validate(highEndPayload)).toBe(true);

    const midRangePayload: HardwareProfileJson = {
      cpu_info: { model: 'AMD Ryzen 5 5600H', cores: 6, threads: 12 },
      ram_info: { total_mb: 16384, available_mb: 10000 },
      gpu_info: {
        vendor: 'NVIDIA',
        model: 'GeForce RTX 3050',
        vram_mb: 4096,
        cuda_supported: true,
      },
      recommended_mode: 'hybrid',
      timestamp: '2026-11-09T20:01:00Z',
    };
    expect(validate(midRangePayload)).toBe(true);

    const lowEndPayload: HardwareProfileJson = {
      cpu_info: { model: 'Intel Core i5-1135G7', cores: 4, threads: 8 },
      ram_info: { total_mb: 8192, available_mb: 3500 },
      gpu_info: {
        vendor: 'Intel',
        model: 'Intel Iris Xe Graphics',
        vram_mb: 0,
        cuda_supported: false,
      },
      recommended_mode: 'cloud_byok',
      recommendation_reason: 'Máy tính văn phòng/iGPU nên dùng Cloud BYOK',
      timestamp: '2026-11-09T20:02:00Z',
    };
    expect(validate(lowEndPayload)).toBe(true);
  });

  it('should validate optional recommendation_reason handling', () => {
    const payloadWithoutReason: HardwareProfileJson = {
      cpu_info: { model: 'Intel i7-13700K', cores: 16, threads: 24 },
      ram_info: { total_mb: 32768, available_mb: 20000 },
      gpu_info: {
        vendor: 'NVIDIA',
        model: 'GeForce RTX 3080',
        vram_mb: 10240,
        cuda_supported: true,
      },
      recommended_mode: 'offline_local',
      timestamp: '2026-11-09T20:00:00Z',
    };
    expect(validate(payloadWithoutReason)).toBe(true);
  });

  it('should reject payload missing top-level required properties', () => {
    // Missing cpu_info
    expect(
      validate({
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        recommended_mode: 'offline_local',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);

    // Missing ram_info
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        recommended_mode: 'offline_local',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);

    // Missing gpu_info
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        recommended_mode: 'offline_local',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);

    // Missing recommended_mode
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);

    // Missing timestamp
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        recommended_mode: 'offline_local',
      }),
    ).toBe(false);
  });

  it('should reject invalid enum values', () => {
    // Unsupported vendor enum "Apple"
    expect(
      validate({
        cpu_info: { model: 'Apple M3 Max', cores: 16, threads: 16 },
        ram_info: { total_mb: 36864, available_mb: 28000 },
        gpu_info: { vendor: 'Apple', model: 'Apple M3 GPU', vram_mb: 36864, cuda_supported: false },
        recommended_mode: 'hybrid',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);

    // Unsupported recommended_mode enum "super_local"
    expect(
      validate({
        cpu_info: { model: 'i7', cores: 8, threads: 16 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        recommended_mode: 'super_local',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);
  });

  it('should reject non-RFC3339 timestamps and invalid data types', () => {
    // Invalid timestamp format
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: 12288, cuda_supported: true },
        recommended_mode: 'hybrid',
        timestamp: '2026/11/09 20:00:00',
      }),
    ).toBe(false);

    // vram_mb is string instead of integer
    expect(
      validate({
        cpu_info: { model: 'i5', cores: 6, threads: 12 },
        ram_info: { total_mb: 16384, available_mb: 8000 },
        gpu_info: { vendor: 'NVIDIA', model: 'RTX 3060', vram_mb: '12288', cuda_supported: true },
        recommended_mode: 'hybrid',
        timestamp: '2026-11-09T20:00:00Z',
      }),
    ).toBe(false);
  });

  it('should verify lossless bidirectional converters and runtime type guards', () => {
    const rawWire: HardwareProfileJson = {
      cpu_info: { model: 'AMD Ryzen 7 5800X', cores: 8, threads: 16 },
      ram_info: { total_mb: 32768, available_mb: 22000 },
      gpu_info: {
        vendor: 'NVIDIA',
        model: 'GeForce RTX 3070',
        vram_mb: 8192,
        cuda_supported: true,
      },
      recommended_mode: 'offline_local',
      recommendation_reason: 'Đạt chuẩn 8GB VRAM và CUDA',
      timestamp: '2026-11-09T20:00:00Z',
    };

    expect(isHardwareProfileJson(rawWire)).toBe(true);

    const domain = toHardwareProfile(rawWire);
    expect(isHardwareProfile(domain)).toBe(true);
    expect(domain.cpuInfo.model).toBe('AMD Ryzen 7 5800X');
    expect(domain.ramInfo.totalMb).toBe(32768);
    expect(domain.gpuInfo.vramMb).toBe(8192);
    expect(domain.recommendedMode).toBe('offline_local');
    expect(domain.recommendationReason).toBe('Đạt chuẩn 8GB VRAM và CUDA');

    const backToWire = toHardwareProfileJson(domain);
    expect(backToWire).toEqual(rawWire);
    expect(isGpuVendor(backToWire.gpu_info.vendor)).toBe(true);
    expect(isRecommendedMode(backToWire.recommended_mode)).toBe(true);
  });
});
