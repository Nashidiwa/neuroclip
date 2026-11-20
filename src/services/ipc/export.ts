import { invoke } from '@tauri-apps/api/core';
import { parseIpcError } from './error.ts';
import {
  type TranscriptSegmentsContainer,
  type SubtitleProject,
  toTranscriptSegmentsContainerJson,
  toSubtitleProjectJson,
} from '../../types/transcript.ts';
import {
  type DualExportResult,
  type CapCutExportOptions,
  type CapCutExportResult,
  normalizeCapCutExportResult,
} from '../../types/export.ts';

export * from '../../types/export.ts';

/**
 * Export transcript segments to an Advanced SubStation Alpha (.ass) file with UTF-8 BOM.
 *
 * @param container Transcript container to export.
 * @param outputPath Target file destination path on disk.
 * @returns Path to the exported subtitle file.
 */
export async function exportSubtitlesAss(
  container: TranscriptSegmentsContainer,
  outputPath: string,
): Promise<string> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    return await invoke<string>('export_subtitles_ass', {
      container: wireContainer,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export transcript segments to a standard SubRip (.srt) subtitle file with UTF-8 BOM.
 *
 * @param container Transcript container to export.
 * @param outputPath Target file destination path on disk.
 * @returns Path to the exported subtitle file.
 */
export async function exportSubtitlesSrt(
  container: TranscriptSegmentsContainer,
  outputPath: string,
): Promise<string> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    return await invoke<string>('export_subtitles_srt', {
      container: wireContainer,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export and persist full subtitle_project.json adhering to Schema 5.6.
 *
 * @param project Subtitle project instance to persist.
 * @param outputPath Target file destination path on disk.
 * @returns Path to the exported project JSON file.
 */
export async function exportSubtitleProject(
  project: SubtitleProject,
  outputPath: string,
): Promise<string> {
  try {
    const wireProject = toSubtitleProjectJson(project);
    return await invoke<string>('export_subtitle_project', {
      project: wireProject,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * ADR-005 Dual Export Guarantee: Exports both ASS and SRT simultaneously.
 *
 * @param container Transcript container to export.
 * @param basePathWithoutExt Base filepath without extension (e.g. "/path/to/subtitles").
 * @returns DualExportResult containing paths to both generated files.
 */
export async function exportSubtitlesDual(
  container: TranscriptSegmentsContainer,
  basePathWithoutExt: string,
): Promise<DualExportResult> {
  const assPath = `${basePathWithoutExt}.ass`;
  const srtPath = `${basePathWithoutExt}.srt`;

  const [savedAss, savedSrt] = await Promise.all([
    exportSubtitlesAss(container, assPath),
    exportSubtitlesSrt(container, srtPath),
  ]);

  return { assPath: savedAss, srtPath: savedSrt };
}

/**
 * Export a SubtitleProject to a CapCut PC Draft project directory with Dual Export Safeguard (ADR-005).
 *
 * Automatically creates:
 * - draft_content.json (Track 0: Video, Track 1: Audio, Track 2: Text)
 * - draft_meta_info.json (draft_materials manifest)
 * - subtitle.ass (Dual Safeguard - Aegisub)
 * - subtitle.srt (Dual Safeguard - SubRip)
 *
 * @param project Subtitle project to export.
 * @param options Optional export configuration (customDraftRoot, videoFilePath override).
 * @returns CapCutExportResult with destination path and safeguard files.
 */
export async function exportCapCutDraft(
  project: SubtitleProject,
  options?: CapCutExportOptions,
): Promise<CapCutExportResult> {
  try {
    const wireProject = toSubtitleProjectJson(project);
    const wireOptions = options
      ? {
          video_file_path: options.videoFilePath ?? null,
          custom_draft_root: options.customDraftRoot ?? null,
        }
      : null;

    const raw = await invoke<Record<string, unknown>>('export_capcut_draft', {
      project: wireProject,
      options: wireOptions,
    });

    return normalizeCapCutExportResult(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export an existing SubtitleProject directly to an Advanced SubStation Alpha (.ass) file with UTF-8 BOM.
 */
export async function exportSubtitlesProjectAss(
  project: SubtitleProject,
  outputPath: string,
): Promise<string> {
  try {
    const wireProject = toSubtitleProjectJson(project);
    return await invoke<string>('export_subtitles_project_ass', {
      project: wireProject,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export an existing SubtitleProject directly to a SubRip (.srt) file with UTF-8 BOM.
 */
export async function exportSubtitlesProjectSrt(
  project: SubtitleProject,
  outputPath: string,
): Promise<string> {
  try {
    const wireProject = toSubtitleProjectJson(project);
    return await invoke<string>('export_subtitles_project_srt', {
      project: wireProject,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}
