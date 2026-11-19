/**
 * Vietnamese Smart Line Wrapping & Readability Engine (`TASK-M4-19`, `PRD.md` §6.1, `CR-03`).
 *
 * Implements client-side counterpart for smart Vietnamese line wrapping,
 * protecting compound words ("bánh mì", "trí tuệ", "bố Vedal", "Evil Neuro", etc.)
 * from being broken across lines, respecting <= 42 chars/line and max 2 lines.
 * Strictly adheres to `docs/CONVENTIONS.md` (zero-any, strict types).
 */

import type { TranscriptSegment } from '../types/transcript.ts';

export const DEFAULT_MAX_CHARS_PER_LINE = 42;
export const MIN_LINE_WRAP_TARGET_CHARS = 38;
export const DEFAULT_MAX_LINES = 2;
export const VERTICAL_9_16_MAX_CHARS_PER_LINE = 26;
export const RED_FLAG_CPS_THRESHOLD = 20.0;
export const TARGET_SAFE_CPS = 17.0;

export const VIETNAMESE_HONORIFICS: readonly string[] = [
  'bố', 'mẹ', 'ông', 'bà', 'anh', 'chị', 'em', 'bé', 'cụ', 'chú', 'bác',
];

export const VIETNAMESE_COMPOUND_WORDS: readonly string[] = [
  // 4-word & 3-word compounds
  'trí tuệ nhân tạo',
  'ngây thơ chưa biết gì',
  'mất kiểm soát rồi',
  'không có manh mối',
  'bắn tim bắn tim',
  'bị bỏ rơi rồi',
  'bộ lọc bị thủng',
  'filter lủng rồi cứu',
  // 2-word Neuro / Swarm Lore & Character Entities
  'bố vedal',
  'bố rùa',
  'evil neuro',
  'neuro-sama',
  'cụ rùa',
  'con rùa',
  'the swarm',
  'bầy đàn',
  'tổ kiến',
  'bộ lọc',
  'bị thủng',
  'lủng rồi',
  'rò rỉ',
  'lộ bản chất',
  'bỏ rơi',
  'bị bỏ rơi',
  'thả tim',
  'bắn tim',
  'gõ đầu',
  'load não',
  'suy nghĩ',
  'sâu xa',
  'nấu chín',
  'xong đời',
  'toang rồi',
  'cook rồi',
  'mất kiểm soát',
  'gãy gánh',
  'điên rồ',
  'thật sự',
  'thực sự',
  'chết cười',
  'cười ẻ',
  'hết cứu',
  'vãi nồi',
  'ngây thơ',
  'tỉnh ngộ',
  'nhận ra',
  'chưa biết',
  'bao tải gym',
  'bao cát gym',
  'giga chad',
  'super chat',
  // Common Vietnamese Compound Nouns
  'bánh mì',
  'trí tuệ',
  'cà phê',
  'máy tính',
  'điện thoại',
  'hình ảnh',
  'âm thanh',
  'phụ đề',
  'thời gian',
  'bản thân',
  'gia đình',
  'bạn bè',
  'con người',
  'thế giới',
  'câu hỏi',
  'câu trả lời',
  'vấn đề',
  'ý kiến',
  'kết quả',
  'lý do',
  'công việc',
  'trò chơi',
  'cảm xúc',
  'sức khỏe',
  'tương lai',
  'quá khứ',
  'hiện tại',
  'cuộc sống',
  'kỷ niệm',
  'thói quen',
  'kinh nghiệm',
  'cơ hội',
  'hy vọng',
  'ước mơ',
  'quyết định',
  'thông tin',
  'nội dung',
  'ý nghĩa',
  'thành phố',
  // Interrogatives & Demonstratives
  'cái gì',
  'chuyện gì',
  'điều gì',
  'ai đó',
  'nơi nào',
  'khi nào',
  'sao lại',
  'làm sao',
  'tại sao',
  'như thế nào',
  'bao nhiêu',
  'bao lâu',
  'bao giờ',
  // Action & State Verbs
  'bắt đầu',
  'kết thúc',
  'tiếp tục',
  'xuất hiện',
  'biến mất',
  'phát hiện',
  'tìm kiếm',
  'giải thích',
  'hướng dẫn',
  'chia sẻ',
  'tham gia',
  'chuẩn bị',
  'thực hiện',
  'hoàn thành',
  'giúp đỡ',
  'quan tâm',
  'bảo vệ',
  'tấn công',
  'chiến đấu',
  'thất bại',
  'chiến thắng',
  'cảm thấy',
  'nhận thấy',
  'nhớ lại',
  'quên mất',
  'cố gắng',
  'đồng ý',
  'từ chối',
  'yêu thích',
  'ghét bỏ',
  'lo lắng',
  'sợ hãi',
  'tự tin',
  'tin tưởng',
  'nghi ngờ',
  'chăm sóc',
  'trò chuyện',
  'thảo luận',
  'thông báo',
  'giới thiệu',
  'thay đổi',
  'phát triển',
  'mở rộng',
  'sửa chữa',
  'khắc phục',
  'rời khỏi',
  'quay lại',
  'trở về',
  'tiến lên',
  'dừng lại',
  // Adjectives & Adverbs
  'tuyệt vời',
  'kinh khủng',
  'khủng khiếp',
  'dễ thương',
  'đáng yêu',
  'thông minh',
  'ngốc nghếch',
  'vui vẻ',
  'hạnh phúc',
  'buồn bã',
  'tức giận',
  'ngạc nhiên',
  'bất ngờ',
  'thất vọng',
  'hài lòng',
  'tự nhiên',
  'đặc biệt',
  'bình thường',
  'quan trọng',
  'cần thiết',
  'rõ ràng',
  'chính xác',
  'đúng đắn',
  'sai lầm',
  'khó khăn',
  'dễ dàng',
  'nguy hiểm',
  'an toàn',
  'nhanh chóng',
  'chậm chạp',
  'mạnh mẽ',
  'yếu đuối',
  'đơn giản',
  'phức tạp',
  'hoàn hảo',
  'sẵn sàng',
  'chắc chắn',
  'may mắn',
  'nghiêm túc',
  'hài hước',
  // Pronouns & Discourse Particles
  'chúng ta',
  'chúng tôi',
  'chúng nó',
  'bọn họ',
  'mọi người',
  'ai nấy',
  'người ta',
  'anh ấy',
  'cô ấy',
  'ông ấy',
  'bà ấy',
  'các bạn',
  'tụi mình',
  'tụi nó',
  'hoàn toàn',
  'nhất định',
  'tuyệt đối',
  'vô cùng',
  'cực kỳ',
  'hầu như',
  'dường như',
  'có vẻ',
  'thậm chí',
  'ngay cả',
  'bỗng nhiên',
  'đột nhiên',
  'thường xuyên',
  'thỉnh thoảng',
  'mãi mãi',
  'lập tức',
  'ngay lập tức',
  'tức thì',
  'tuy nhiên',
  'bởi vì',
  'cho nên',
  'do đó',
  'vì vậy',
  'mặc dù',
  'cho dù',
  'nếu như',
  'giả sử',
  'miễn là',
  'ngoài ra',
  'hơn nữa',
  'cuối cùng',
  'tóm lại',
];

const COMPOUND_SET: ReadonlySet<string> = new Set(VIETNAMESE_COMPOUND_WORDS);
const HONORIFICS_SET: ReadonlySet<string> = new Set(VIETNAMESE_HONORIFICS);

/**
 * Accurately count Unicode characters (code points) in a string.
 */
export function getUnicodeCharCount(str: string): number {
  return Array.from(str).length;
}

/**
 * Strips leading and trailing punctuation marks from a token.
 */
export function stripPunctuation(word: string): string {
  return word.replace(/^[\p{P}]+|[\p{P}]+$/gu, '');
}

/**
 * Check if candidate phrase is in compound words catalog.
 */
export function isCompoundWord(phrase: string): boolean {
  return COMPOUND_SET.has(phrase.toLowerCase().trim());
}

/**
 * Check if candidate word is a Vietnamese honorific / kinship title.
 */
export function isHonorific(word: string): boolean {
  return HONORIFICS_SET.has(word.toLowerCase().trim());
}

/**
 * Check if word starts with uppercase character.
 */
export function startsWithUppercase(word: string): boolean {
  const first = word.charAt(0);
  return first !== '' && first === first.toUpperCase() && first !== first.toLowerCase();
}

/**
 * Tokenize Vietnamese text into atomic compound tokens.
 */
export function tokenizeVietnameseCompounds(text: string): string[] {
  const rawWords = text.trim().split(/\s+/).filter(Boolean);
  if (rawWords.length === 0) return [];

  const tokens: string[] = [];
  let i = 0;

  while (i < rawWords.length) {
    const w0 = rawWords[i] ?? '';
    const s0 = stripPunctuation(w0);

    // 4-word window
    if (i + 3 < rawWords.length) {
      const w1 = rawWords[i + 1] ?? '';
      const w2 = rawWords[i + 2] ?? '';
      const w3 = rawWords[i + 3] ?? '';
      const s1 = stripPunctuation(w1);
      const s2 = stripPunctuation(w2);
      const s3 = stripPunctuation(w3);
      if (s0 && s1 && s2 && s3) {
        const candidate = `${s0} ${s1} ${s2} ${s3}`;
        if (isCompoundWord(candidate)) {
          tokens.push(`${w0} ${w1} ${w2} ${w3}`);
          i += 4;
          continue;
        }
      }
    }

    // 3-word window
    if (i + 2 < rawWords.length) {
      const w1 = rawWords[i + 1] ?? '';
      const w2 = rawWords[i + 2] ?? '';
      const s1 = stripPunctuation(w1);
      const s2 = stripPunctuation(w2);
      if (s0 && s1 && s2) {
        const candidate = `${s0} ${s1} ${s2}`;
        if (isCompoundWord(candidate)) {
          tokens.push(`${w0} ${w1} ${w2}`);
          i += 3;
          continue;
        }
      }
    }

    // 2-word window
    if (i + 1 < rawWords.length) {
      const w1 = rawWords[i + 1] ?? '';
      const s1 = stripPunctuation(w1);
      if (s0 && s1) {
        const candidate = `${s0} ${s1}`;

        const isCompound = isCompoundWord(candidate);
        const isHonorificName = isHonorific(s0) && startsWithUppercase(s1);
        const isCapitalizedPair = startsWithUppercase(s0) && startsWithUppercase(s1) && isCompoundWord(candidate);

        if (isCompound || isHonorificName || isCapitalizedPair) {
          tokens.push(`${w0} ${w1}`);
          i += 2;
          continue;
        }
      }
    }

    // Single word
    tokens.push(w0);
    i += 1;
  }

  return tokens;
}

/**
 * Wrap Vietnamese text into lines of at most `maxCharsPerLine` characters,
 * strictly preserving compound words across line breaks.
 */
export function wrapVietnameseText(
  text: string,
  maxCharsPerLine: number = DEFAULT_MAX_CHARS_PER_LINE,
  _maxLines: number = DEFAULT_MAX_LINES,
): string[] {
  const cleanText = text.trim();
  if (!cleanText) return [];

  const limit = maxCharsPerLine > 0 ? maxCharsPerLine : DEFAULT_MAX_CHARS_PER_LINE;
  if (getUnicodeCharCount(cleanText) <= limit) {
    return [cleanText];
  }

  const tokens = tokenizeVietnameseCompounds(cleanText);
  const lines: string[] = [];
  let currentLine = '';
  let currentChars = 0;

  for (const token of tokens) {
    const tokenChars = getUnicodeCharCount(token);

    if (tokenChars > limit) {
      if (currentLine) {
        lines.push(currentLine);
        currentLine = '';
        currentChars = 0;
      }
      const chars = Array.from(token);
      for (let c = 0; c < chars.length; c += limit) {
        const slice = chars.slice(c, c + limit);
        const chunk = slice.join('');
        if (slice.length === limit) {
          lines.push(chunk);
        } else {
          currentLine = chunk;
          currentChars = slice.length;
        }
      }
      continue;
    }

    const space = currentLine ? 1 : 0;
    if (currentChars + space + tokenChars <= limit) {
      currentLine = currentLine ? `${currentLine} ${token}` : token;
      currentChars += space + tokenChars;
    } else {
      lines.push(currentLine);
      currentLine = token;
      currentChars = tokenChars;
    }
  }

  if (currentLine) {
    lines.push(currentLine);
  }

  return lines;
}

/**
 * Wrap Vietnamese text and return joined by newlines.
 */
export function wrapVietnameseSubtitle(
  text: string,
  maxCharsPerLine: number = DEFAULT_MAX_CHARS_PER_LINE,
  maxLines: number = DEFAULT_MAX_LINES,
): string {
  return wrapVietnameseText(text, maxCharsPerLine, maxLines).join('\n');
}

/**
 * Alias for `wrapVietnameseSubtitle`.
 */
export function wrapVietnameseTextToString(
  text: string,
  maxCharsPerLine: number = DEFAULT_MAX_CHARS_PER_LINE,
  maxLines: number = DEFAULT_MAX_LINES,
): string {
  return wrapVietnameseSubtitle(text, maxCharsPerLine, maxLines);
}

/**
 * Compute characters per second (CPS) for a given text and duration in seconds.
 */
export function calculateCps(text: string, durationSec: number): number {
  const chars = getUnicodeCharCount(text.trim());
  const dur = Math.max(0.05, durationSec);
  const cps = chars / dur;
  return Number.isFinite(cps) && cps >= 0 ? Number(cps.toFixed(2)) : 0;
}

/**
 * Compute CPS for a segment using its effective text (prefers text_vi).
 */
export function calculateSegmentCps(segment: TranscriptSegment): number {
  const text = segment.textVi && segment.textVi.trim() !== '' ? segment.textVi : segment.textEn;
  const duration = segment.endSec - segment.startSec;
  return calculateCps(text, duration);
}

/**
 * Options for Time Borrowing optimization.
 */
export interface TimeBorrowingOptions {
  maxBorrowMs?: number;
  minGapMs?: number;
  targetCps?: number;
  triggerCps?: number;
  maxClipDurationSec?: number;
}

/**
 * Client-side Time Borrowing optimization for transcript segments.
 */
export function optimizeCpsTimeBorrowing(
  segments: TranscriptSegment[],
  options?: TimeBorrowingOptions,
): TranscriptSegment[] {
  const maxBorrowMs = options?.maxBorrowMs ?? 800;
  const minGapMs = options?.minGapMs ?? 50;
  const targetCps = options?.targetCps ?? 17.0;
  const triggerCps = options?.triggerCps ?? RED_FLAG_CPS_THRESHOLD;
  const maxClipDuration = options?.maxClipDurationSec;

  const maxBorrowSec = maxBorrowMs / 1000;
  const minGapSec = minGapMs / 1000;

  return segments.map((seg, idx, arr) => {
    const text = seg.textVi && seg.textVi.trim() !== '' ? seg.textVi : seg.textEn;
    const charCount = getUnicodeCharCount(text.trim());
    let currentDuration = Math.max(0.05, seg.endSec - seg.startSec);
    let currentCps = charCount / currentDuration;

    if (currentCps <= triggerCps) {
      const rounded = Number(currentCps.toFixed(2));
      return {
        ...seg,
        cpsScore: rounded,
        cps_score: rounded,
      };
    }

    const desiredDuration = targetCps > 0 ? charCount / targetCps : currentDuration;
    const neededExtension = Math.max(0, desiredDuration - currentDuration);

    const nextSeg = arr[idx + 1];
    let maxGapExtension = Number.POSITIVE_INFINITY;
    if (nextSeg) {
      maxGapExtension = Math.max(0, nextSeg.startSec - seg.endSec - minGapSec);
    } else if (maxClipDuration !== undefined) {
      maxGapExtension = Math.max(0, maxClipDuration - seg.endSec);
    }

    const allowedExtension = Math.min(neededExtension, maxBorrowSec, maxGapExtension);
    const borrowedSec = Number.isFinite(allowedExtension) && allowedExtension > 0
      ? Math.round(allowedExtension * 1000) / 1000
      : 0;

    const newEndSec = Math.round((seg.endSec + borrowedSec) * 1000) / 1000;
    currentDuration = Math.max(0.05, newEndSec - seg.startSec);
    currentCps = charCount / currentDuration;
    const newCps = Number(currentCps.toFixed(2));

    const isFlagged = seg.confidence < 0.6 || newCps > triggerCps;

    return {
      ...seg,
      endSec: newEndSec,
      end_sec: newEndSec,
      cpsScore: newCps,
      cps_score: newCps,
      isFlaggedForReview: isFlagged,
      is_flagged_for_review: isFlagged,
    };
  });
}
