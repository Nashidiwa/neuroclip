import { describe, it, expect } from 'vitest';
import {
  calculateCps,
  getUnicodeCharCount,
  isCompoundWord,
  isHonorific,
  optimizeCpsTimeBorrowing,
  RED_FLAG_CPS_THRESHOLD,
  stripPunctuation,
  tokenizeVietnameseCompounds,
  VERTICAL_9_16_MAX_CHARS_PER_LINE,
  wrapVietnameseSubtitle,
  wrapVietnameseText,
} from '../utils/text_wrap.ts';
import { normalizeTranscriptSegment, type TranscriptSegment } from '../types/transcript.ts';

describe('TASK-M4-19: Vietnamese Smart Line Wrapping & CPS Optimization', () => {
  describe('UT-19.1: Smart Compound Line Wrapping', () => {
    it('should wrap 50-char string containing "bánh mì" and "bố Vedal" without breaking compounds', () => {
      const text50 = 'Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.';
      expect(getUnicodeCharCount(text50)).toBe(50);

      const lines = wrapVietnameseText(text50, 42, 2);
      expect(lines).toHaveLength(2);

      for (const line of lines) {
        expect(getUnicodeCharCount(line)).toBeLessThanOrEqual(42);
      }

      // Assert that line breaks never split "bánh mì" or "bố Vedal"
      expect(lines[0]?.endsWith('bánh')).toBe(false);
      expect(lines[0]?.endsWith('bố')).toBe(false);
      expect(lines[0]).toBe('Tôi đã mua một ổ bánh mì rất ngon cho');
      expect(lines[1]).toBe('bố Vedal ăn.');
    });

    it('should keep short text in a single line', () => {
      const text = 'Chào bố Vedal!';
      const lines = wrapVietnameseText(text, 42, 2);
      expect(lines).toHaveLength(1);
      expect(lines[0]).toBe('Chào bố Vedal!');
    });

    it('should keep exactly 42 characters in a single line', () => {
      const text42 = 'Một hai ba bốn năm sáu bảy tám chín mười!!';
      expect(getUnicodeCharCount(text42)).toBe(42);

      const lines = wrapVietnameseText(text42, 42, 2);
      expect(lines).toHaveLength(1);
      expect(lines[0]).toBe(text42);
    });

    it('should wrap 43 characters into 2 lines', () => {
      const text43 = 'Một hai ba bốn năm sáu bảy tám chín mười!!!';
      expect(getUnicodeCharCount(text43)).toBe(43);

      const lines = wrapVietnameseText(text43, 42, 2);
      expect(lines).toHaveLength(2);
      for (const line of lines) {
        expect(getUnicodeCharCount(line)).toBeLessThanOrEqual(42);
      }
    });

    it('should preserve multiple compounds and named entities intact', () => {
      const text = 'Evil Neuro cùng bố Vedal uống cà phê và ăn bánh mì.';
      const lines = wrapVietnameseText(text, 42, 2);
      expect(lines).toHaveLength(2);
      for (const line of lines) {
        expect(getUnicodeCharCount(line)).toBeLessThanOrEqual(42);
        expect(line.endsWith('Evil')).toBe(false);
        expect(line.endsWith('bố')).toBe(false);
        expect(line.endsWith('cà')).toBe(false);
        expect(line.endsWith('bánh')).toBe(false);
      }
    });

    it('should enforce 26-char limit for 9:16 vertical video layout', () => {
      const text = 'Tôi thích ăn bánh mì và uống cà phê cùng bố Vedal.';
      const lines = wrapVietnameseText(text, VERTICAL_9_16_MAX_CHARS_PER_LINE, 2);
      for (const line of lines) {
        expect(getUnicodeCharCount(line)).toBeLessThanOrEqual(26);
        expect(line.endsWith('bánh')).toBe(false);
        expect(line.endsWith('cà')).toBe(false);
        expect(line.endsWith('bố')).toBe(false);
      }
    });

    it('should handle empty or whitespace-only strings safely', () => {
      expect(wrapVietnameseText('')).toEqual([]);
      expect(wrapVietnameseText('   \n\t  ')).toEqual([]);
    });

    it('should join wrapped lines with newlines via wrapVietnameseSubtitle', () => {
      const text50 = 'Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.';
      const subtitle = wrapVietnameseSubtitle(text50, 42, 2);
      expect(subtitle).toBe('Tôi đã mua một ổ bánh mì rất ngon cho\nbố Vedal ăn.');
    });

    it('should wrap unbroken long ASCII token into chunks without data loss', () => {
      const longAscii = 'A'.repeat(50);
      const lines = wrapVietnameseText(longAscii, 42, 2);
      expect(lines).toHaveLength(2);
      expect(getUnicodeCharCount(lines[0] ?? '')).toBe(42);
      expect(getUnicodeCharCount(lines[1] ?? '')).toBe(8);
      expect((lines[0] ?? '') + (lines[1] ?? '')).toBe(longAscii);
    });

    it('should wrap unbroken long non-BMP / emoji token preserving all emojis without data loss', () => {
      const emojis50 = '💀'.repeat(50);
      expect(getUnicodeCharCount(emojis50)).toBe(50);

      const lines = wrapVietnameseText(emojis50, 42, 2);
      expect(lines).toHaveLength(2);
      expect(getUnicodeCharCount(lines[0] ?? '')).toBe(42);
      expect(getUnicodeCharCount(lines[1] ?? '')).toBe(8);
      expect((lines[0] ?? '') + (lines[1] ?? '')).toBe(emojis50);

      const totalPreserved = lines.reduce((acc, l) => acc + getUnicodeCharCount(l), 0);
      expect(totalPreserved).toBe(50);
    });

    it('should correctly handle emoji token exact multiples of limit (84 emojis)', () => {
      const emojis84 = '💀'.repeat(84);
      const lines = wrapVietnameseText(emojis84, 42, 2);
      expect(lines).toHaveLength(2);
      expect(getUnicodeCharCount(lines[0] ?? '')).toBe(42);
      expect(getUnicodeCharCount(lines[1] ?? '')).toBe(42);
      expect((lines[0] ?? '') + (lines[1] ?? '')).toBe(emojis84);
    });

    it('should preserve correct line length tracking when words follow a long emoji token', () => {
      // 50 emojis + space + 8-char word ('kết thúc')
      // Line 1: 42 emojis
      // Line 2: 8 emojis + 1 space + 8 chars = 17 chars (fits <= 42)
      const text = '💀'.repeat(50) + ' kết thúc';
      const lines = wrapVietnameseText(text, 42, 2);
      expect(lines).toHaveLength(2);
      expect(getUnicodeCharCount(lines[0] ?? '')).toBe(42);
      expect(getUnicodeCharCount(lines[1] ?? '')).toBe(17);
      expect((lines[1] ?? '').endsWith('kết thúc')).toBe(true);
    });

    it('should maintain parity with Rust when wrapping compounds adjacent to long emoji tokens', () => {
      const text = 'bánh mì và bố Vedal ' + '💀'.repeat(50);
      const lines = wrapVietnameseText(text, 42, 2);

      expect(lines).toHaveLength(3);
      // Line 1: 'bánh mì và bố Vedal' (19 characters)
      expect(lines[0]).toBe('bánh mì và bố Vedal');
      expect(getUnicodeCharCount(lines[0] ?? '')).toBe(19);

      // Line 2: '💀'.repeat(42) (42 characters)
      expect(lines[1]).toBe('💀'.repeat(42));
      expect(getUnicodeCharCount(lines[1] ?? '')).toBe(42);

      // Line 3: '💀'.repeat(8) (8 characters)
      expect(lines[2]).toBe('💀'.repeat(8));
      expect(getUnicodeCharCount(lines[2] ?? '')).toBe(8);
    });
  });

  describe('UT-19.2: CPS Reading Speed Calculation', () => {
    it('should calculate CPS correctly and detect red-flag reading speed (> 20 CPS)', () => {
      // 40 Vietnamese characters in 1.5 seconds -> CPS = 40 / 1.5 ≈ 26.67
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      expect(getUnicodeCharCount(text40)).toBe(40);

      const cps = calculateCps(text40, 1.5);
      expect(cps).toBeCloseTo(26.67, 1);
      expect(cps).toBeGreaterThan(RED_FLAG_CPS_THRESHOLD);
    });

    it('should clamp duration to minimum 0.05s to prevent division by zero', () => {
      const cps = calculateCps('Hello', 0);
      expect(cps).toBe(100); // 5 / 0.05 = 100
      expect(Number.isFinite(cps)).toBe(true);
    });
  });

  describe('UT-19.3 & ED-19.1: Time Borrowing & Collision Avoidance', () => {
    function createMockSegment(
      id: string,
      startSec: number,
      endSec: number,
      textEn: string,
      textVi: string,
      confidence: number = 0.95,
    ): TranscriptSegment {
      const cps = calculateCps(textVi, endSec - startSec);
      return normalizeTranscriptSegment({
        segment_id: id,
        start_sec: startSec,
        end_sec: endSec,
        speaker_id: 'neuro',
        text_en: textEn,
        text_vi: textVi,
        confidence,
        cps_score: cps,
        is_flagged_for_review: confidence < 0.6 || cps > RED_FLAG_CPS_THRESHOLD,
        words: [],
      });
    }

    it('UT-19.3: should borrow up to 0.8s into a 1.0s gap to reduce CPS to <= 20.0', () => {
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      const segments: TranscriptSegment[] = [
        createMockSegment('seg_1', 0.0, 1.5, 'Forty chars sentence in English text!', text40, 0.95),
        createMockSegment('seg_2', 2.5, 4.0, 'Next sentence', 'Câu tiếp theo', 0.95),
      ];

      const optimized = optimizeCpsTimeBorrowing(segments);
      const seg1 = optimized[0];
      const seg2 = optimized[1];

      expect(seg1).toBeDefined();
      expect(seg2).toBeDefined();
      if (!seg1 || !seg2) return;

      // endSec must be extended by 0.8s (1.5 + 0.8 = 2.3s)
      expect(seg1.endSec).toBeCloseTo(2.3, 2);
      // New CPS: 40 / 2.3 ≈ 17.39
      expect(seg1.cpsScore).toBeCloseTo(17.39, 1);
      expect(seg1.cpsScore).toBeLessThanOrEqual(RED_FLAG_CPS_THRESHOLD);
      expect(seg1.isFlaggedForReview).toBe(false);

      // Distance to next start must be >= 50ms buffer (2.5 - 2.3 = 0.2s = 200ms >= 50ms)
      expect(seg2.startSec - seg1.endSec).toBeGreaterThanOrEqual(0.05);
    });

    it('ED-19.1: should preserve at least 50ms safety gap when silence gap is short', () => {
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      const segments: TranscriptSegment[] = [
        createMockSegment('seg_1', 0.0, 1.5, 'English text', text40, 0.95),
        createMockSegment('seg_2', 1.6, 3.0, 'Next sentence', 'Câu tiếp theo', 0.95), // 100ms gap
      ];

      const optimized = optimizeCpsTimeBorrowing(segments);
      const seg1 = optimized[0];
      const seg2 = optimized[1];

      expect(seg1).toBeDefined();
      expect(seg2).toBeDefined();
      if (!seg1 || !seg2) return;

      // With 100ms gap, max borrow is 100ms - 50ms = 50ms -> endSec = 1.55s
      expect(seg1.endSec).toBeCloseTo(1.55, 2);
      expect(seg2.startSec - seg1.endSec).toBeCloseTo(0.05, 3);
      expect(seg1.endSec).toBeLessThanOrEqual(seg2.startSec - 0.05);
    });

    it('should not borrow when silence gap is strictly less than 50ms', () => {
      const text40 = 'Một hai ba bốn năm sáu bảy tám chín mười';
      const segments: TranscriptSegment[] = [
        createMockSegment('seg_1', 0.0, 1.5, 'English text', text40, 0.95),
        createMockSegment('seg_2', 1.53, 3.0, 'Next sentence', 'Câu tiếp', 0.95), // 30ms gap (< 50ms)
      ];

      const optimized = optimizeCpsTimeBorrowing(segments);
      const seg1 = optimized[0];

      expect(seg1).toBeDefined();
      if (!seg1) return;

      expect(seg1.endSec).toBe(1.5);
      expect(seg1.isFlaggedForReview).toBe(true);
    });
  });

  describe('Helper Functions & Vocabulary Recognition', () => {
    it('should correctly strip leading and trailing punctuation', () => {
      expect(stripPunctuation('"bánh mì,"')).toBe('bánh mì');
      expect(stripPunctuation('...trí tuệ!?')).toBe('trí tuệ');
      expect(stripPunctuation('«Cụ Rùa»')).toBe('Cụ Rùa');
      expect(stripPunctuation('💀')).toBe('💀');
      expect(stripPunctuation('💀💀💀')).toBe('💀💀💀');
      expect(stripPunctuation('!💀!')).toBe('💀');
    });

    it('should tokenize emojis and compound words cleanly without accidental fusion', () => {
      expect(tokenizeVietnameseCompounds('bánh mì 💀')).toEqual(['bánh mì', '💀']);
      expect(tokenizeVietnameseCompounds('💀 bánh mì')).toEqual(['💀', 'bánh mì']);
    });

    it('should recognize compounds and honorifics', () => {
      expect(isCompoundWord('bánh mì')).toBe(true);
      expect(isCompoundWord('bố vedal')).toBe(true);
      expect(isCompoundWord('evil neuro')).toBe(true);
      expect(isCompoundWord('random word')).toBe(false);

      expect(isHonorific('bố')).toBe(true);
      expect(isHonorific('cụ')).toBe(true);
      expect(isHonorific('bé')).toBe(true);
      expect(isHonorific('máy')).toBe(false);
    });

    it('should tokenize multi-word compound phrases', () => {
      const tokens = tokenizeVietnameseCompounds('Hôm nay Evil Neuro ăn bánh mì cùng bố Vedal.');
      expect(tokens).toContain('Evil Neuro');
      expect(tokens).toContain('bánh mì');
      expect(tokens).toContain('bố Vedal.');
    });
  });
});
