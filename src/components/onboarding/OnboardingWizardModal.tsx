import React, { useState } from 'react';
import {
  Sparkles,
  Zap,
  Target,
  FileVideo,
  ShieldCheck,
  ChevronRight,
  ChevronLeft,
  X,
  Check,
  CheckCircle2,
} from 'lucide-react';
import { HardwareRecommendation } from './HardwareRecommendation.tsx';
import { ApiKeyTester } from './ApiKeyTester.tsx';
import { useSettingsStore } from '../../stores/settingsStore.ts';
import { useHardwareStore } from '../../stores/useHardwareStore.ts';
import { setApiKey } from '../../services/config/settings.ts';

export interface OnboardingWizardModalProps {
  readonly isOpen: boolean;
  readonly onClose: () => void;
  readonly onComplete?: (() => void) | undefined;
}

export type WizardStep = 1 | 2 | 3 | 4;

export const OnboardingWizardModal: React.FC<OnboardingWizardModalProps> = ({
  isOpen,
  onClose,
  onComplete,
}) => {
  const [step, setStep] = useState<WizardStep>(1);
  const [groqKey, setGroqKey] = useState<string>('');
  const [geminiKey, setGeminiKey] = useState<string>('');
  const [isFinishing, setIsFinishing] = useState<boolean>(false);

  const completeOnboardingStore = useSettingsStore((state) => state.completeOnboarding);
  const selectedMode = useHardwareStore((state) => state.selectedMode);

  if (!isOpen) {
    return null;
  }

  const handleFinish = async (): Promise<void> => {
    setIsFinishing(true);
    try {
      // Save entered keys to Keyring if non-empty
      if (groqKey.trim()) {
        try {
          await setApiKey('groq', groqKey.trim());
        } catch {
          // Fallback or ignore
        }
      }
      if (geminiKey.trim()) {
        try {
          await setApiKey('gemini', geminiKey.trim());
        } catch {
          // Fallback or ignore
        }
      }

      // Mark onboarding complete in settingsStore & atomic config
      await completeOnboardingStore();

      onComplete?.();
      onClose();
    } finally {
      setIsFinishing(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="wizard-title"
      className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-black/80 p-4 backdrop-blur-sm"
    >
      <div className="text-text-primary relative flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-border-dark bg-bg-primary shadow-acrylic">
        {/* Header with Title and Stepper */}
        <div className="flex items-center justify-between border-b border-border-dark bg-bg-secondary px-6 py-4">
          <div className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-accent-neuro" />
            <h2 id="wizard-title" className="text-text-primary text-base font-bold">
              Chào Mừng Đến Với NeuroClip Studio
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-text-muted hover:text-text-primary rounded p-1 transition-colors hover:bg-bg-tertiary"
            title="Đóng trình hướng dẫn"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Stepper Indicator */}
        <div className="flex items-center justify-between border-b border-border-dark/60 bg-bg-primary px-8 py-3 text-xs">
          {[
            { stepNum: 1, label: 'Giới thiệu' },
            { stepNum: 2, label: 'Phần cứng' },
            { stepNum: 3, label: 'Khóa API' },
            { stepNum: 4, label: 'Hoàn tất' },
          ].map(({ stepNum, label }, idx) => (
            <React.Fragment key={stepNum}>
              <div className="flex items-center gap-2">
                <span
                  className={`flex h-6 w-6 items-center justify-center rounded-full text-xs font-bold transition-colors ${
                    step === stepNum
                      ? 'bg-accent-neuro text-bg-primary shadow-sm'
                      : step > stepNum
                        ? 'bg-accent-vedal text-bg-primary'
                        : 'text-text-muted bg-bg-tertiary'
                  }`}
                >
                  {step > stepNum ? <Check className="h-3.5 w-3.5 stroke-[3]" /> : stepNum}
                </span>
                <span
                  className={`font-medium ${
                    step === stepNum
                      ? 'text-accent-neuro'
                      : step > stepNum
                        ? 'text-accent-vedal'
                        : 'text-text-muted'
                  }`}
                >
                  {label}
                </span>
              </div>
              {idx < 3 && <div className="h-0.5 w-8 bg-border-dark" />}
            </React.Fragment>
          ))}
        </div>

        {/* Body Steps */}
        <div className="flex-1 overflow-y-auto p-6">
          {/* STEP 1: Welcome & Highlights */}
          {step === 1 && (
            <div className="space-y-6">
              <div className="text-center">
                <span className="inline-flex items-center gap-1.5 rounded-full border border-accent-neuro/30 bg-accent-neuro/10 px-3 py-1 text-xs font-semibold text-accent-neuro">
                  <Sparkles className="h-3.5 w-3.5" />
                  <span>Dành riêng cho Clipper & Subber Neuro-sama</span>
                </span>
                <h3 className="text-text-primary mt-3 text-xl font-bold tracking-tight">
                  Bộ công cụ AI cắt clip & làm phụ đề tự động siêu tốc
                </h3>
                <p className="text-text-muted mt-1 text-sm">
                  Tối ưu hóa quy trình sản xuất video highlight với độ trễ tối thiểu, bảo mật tuyệt
                  đối.
                </p>
              </div>

              <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                <div className="space-y-2 rounded-lg border border-border-dark bg-bg-secondary p-4">
                  <div className="flex h-8 w-8 items-center justify-center rounded bg-accent-chat/10 text-accent-chat">
                    <Zap className="h-4 w-4" />
                  </div>
                  <h4 className="text-text-primary text-sm font-semibold">Audio-First Ingestion</h4>
                  <p className="text-text-muted text-xs">
                    Thu nạp luồng âm thanh PCM trực tiếp từ VOD. Không cần tải trước 30GB tệp video
                    thô.
                  </p>
                </div>

                <div className="space-y-2 rounded-lg border border-border-dark bg-bg-secondary p-4">
                  <div className="flex h-8 w-8 items-center justify-center rounded bg-accent-neuro/10 text-accent-neuro">
                    <Target className="h-4 w-4" />
                  </div>
                  <h4 className="text-text-primary text-sm font-semibold">Multimodal Highlights</h4>
                  <p className="text-text-muted text-xs">
                    Bắt trọn khoảnh khắc hài hước bằng thuật toán kết hợp Chat Velocity, Emote Swarm
                    & RMS.
                  </p>
                </div>

                <div className="space-y-2 rounded-lg border border-border-dark bg-bg-secondary p-4">
                  <div className="flex h-8 w-8 items-center justify-center rounded bg-accent-vedal/10 text-accent-vedal">
                    <FileVideo className="h-4 w-4" />
                  </div>
                  <h4 className="text-text-primary text-sm font-semibold">Theatrical Subtitles</h4>
                  <p className="text-text-muted text-xs">
                    Phụ đề ngữ cảnh cảm xúc, xuất nháp CapCut Draft và Premiere XML sẵn sàng biên
                    tập ngay.
                  </p>
                </div>
              </div>

              <div className="flex justify-end border-t border-border-dark pt-4">
                <button
                  type="button"
                  onClick={() => setStep(2)}
                  className="inline-flex items-center gap-2 rounded-lg bg-accent-neuro px-4 py-2 text-sm font-bold text-bg-primary shadow transition-all hover:brightness-110 active:scale-95"
                >
                  <span>Bắt đầu thiết lập</span>
                  <ChevronRight className="h-4 w-4" />
                </button>
              </div>
            </div>
          )}

          {/* STEP 2: Hardware Scan & Recommendation */}
          {step === 2 && (
            <div className="space-y-4">
              <div className="mb-2">
                <h3 className="text-text-primary text-base font-bold">
                  Đánh giá năng lực phần cứng & đề xuất chế độ
                </h3>
                <p className="text-text-muted text-xs">
                  Hệ thống tự động phát hiện CPU, RAM và GPU VRAM để đề xuất chế độ làm việc tối ưu
                  nhất.
                </p>
              </div>

              <HardwareRecommendation compact={false} />

              <div className="flex items-center justify-between border-t border-border-dark pt-4">
                <button
                  type="button"
                  onClick={() => setStep(1)}
                  className="text-text-primary inline-flex items-center gap-1.5 rounded-lg border border-border-dark bg-bg-secondary px-3.5 py-2 text-sm font-medium hover:bg-bg-tertiary"
                >
                  <ChevronLeft className="h-4 w-4" />
                  <span>Quay lại</span>
                </button>
                <button
                  type="button"
                  onClick={() => setStep(3)}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-accent-neuro px-4 py-2 text-sm font-bold text-bg-primary shadow hover:brightness-110 active:scale-95"
                >
                  <span>Tiếp tục (Khóa API)</span>
                  <ChevronRight className="h-4 w-4" />
                </button>
              </div>
            </div>
          )}

          {/* STEP 3: BYOK API Keys Configuration */}
          {step === 3 && (
            <div className="space-y-4">
              <div>
                <h3 className="text-text-primary text-base font-bold">
                  Cấu hình khóa BYOK API Keys (Miễn phí)
                </h3>
                <p className="text-text-muted text-xs">
                  Nhập API Key để kích hoạt tính năng bóc băng Groq Whisper siêu tốc và dịch thuật
                  ngữ cảnh Gemini.
                </p>
              </div>

              <ApiKeyTester
                provider="groq"
                label="Groq Cloud API Key (Whisper STT Siêu Tốc)"
                value={groqKey}
                onChange={setGroqKey}
                getKeyUrl="https://console.groq.com/keys"
                placeholder="gsk_..."
                helpText="Dùng cho bóc băng giọng nói Neuro-sama với mô hình whisper-large-v3 miễn phí."
              />

              <ApiKeyTester
                provider="gemini"
                label="Google Gemini API Key (Dịch Thuật Theatrical)"
                value={geminiKey}
                onChange={setGeminiKey}
                getKeyUrl="https://aistudio.google.com/app/apikey"
                placeholder="AIza..."
                helpText="Dùng cho dịch thuật ngữ cảnh sân khấu Việt hóa chuẩn lore NeuroClip."
              />

              <div className="flex items-start gap-2 rounded-lg border border-accent-vedal/30 bg-accent-vedal/10 p-3 text-xs text-accent-vedal">
                <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" />
                <span>
                  <strong>Bảo mật tuyệt đối:</strong> 100% Khóa API được mã hóa an toàn trong
                  Windows Credential Locker. Tuyệt đối không lưu plain text vào tệp cấu hình hay ổ
                  đĩa (AC-28.2).
                </span>
              </div>

              <div className="flex items-center justify-between border-t border-border-dark pt-4">
                <button
                  type="button"
                  onClick={() => setStep(2)}
                  className="text-text-primary inline-flex items-center gap-1.5 rounded-lg border border-border-dark bg-bg-secondary px-3.5 py-2 text-sm font-medium hover:bg-bg-tertiary"
                >
                  <ChevronLeft className="h-4 w-4" />
                  <span>Quay lại</span>
                </button>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setStep(4)}
                    className="text-text-muted hover:text-text-primary rounded-lg px-3 py-2 text-xs"
                  >
                    Bỏ qua (Thiết lập sau)
                  </button>
                  <button
                    type="button"
                    onClick={() => setStep(4)}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-accent-neuro px-4 py-2 text-sm font-bold text-bg-primary shadow hover:brightness-110 active:scale-95"
                  >
                    <span>Tiếp tục</span>
                    <ChevronRight className="h-4 w-4" />
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* STEP 4: Summary & Ready to Launch */}
          {step === 4 && (
            <div className="space-y-6">
              <div className="text-center">
                <span className="inline-flex h-12 w-12 items-center justify-center rounded-full bg-accent-vedal/20 text-accent-vedal">
                  <CheckCircle2 className="h-6 w-6" />
                </span>
                <h3 className="text-text-primary mt-3 text-xl font-bold">
                  Sẵn Sàng Sáng Tạo Cùng NeuroClip!
                </h3>
                <p className="text-text-muted mt-1 text-sm">
                  Tất cả cấu hình đã được thiết lập thành công. Bạn có thể thay đổi bất kỳ lúc nào
                  trong Cài đặt.
                </p>
              </div>

              <div className="space-y-3 rounded-lg border border-border-dark bg-bg-secondary p-4">
                <h4 className="text-text-muted text-xs font-semibold uppercase tracking-wider">
                  Tóm tắt cấu hình khởi chạy
                </h4>
                <div className="grid grid-cols-1 gap-2 text-xs sm:grid-cols-2">
                  <div className="flex items-center justify-between rounded border border-border-dark bg-bg-primary p-2.5">
                    <span className="text-text-muted">Chế độ làm việc:</span>
                    <span className="font-semibold capitalize text-accent-neuro">
                      {selectedMode ?? 'Hybrid (Groq + Local)'}
                    </span>
                  </div>

                  <div className="flex items-center justify-between rounded border border-border-dark bg-bg-primary p-2.5">
                    <span className="text-text-muted">Groq API Key:</span>
                    <span
                      className={
                        groqKey.trim() ? 'font-semibold text-accent-vedal' : 'text-text-muted'
                      }
                    >
                      {groqKey.trim() ? 'Đã thiết lập (Keyring)' : 'Chưa cấu hình'}
                    </span>
                  </div>

                  <div className="flex items-center justify-between rounded border border-border-dark bg-bg-primary p-2.5">
                    <span className="text-text-muted">Gemini API Key:</span>
                    <span
                      className={
                        geminiKey.trim() ? 'font-semibold text-accent-vedal' : 'text-text-muted'
                      }
                    >
                      {geminiKey.trim() ? 'Đã thiết lập (Keyring)' : 'Chưa cấu hình'}
                    </span>
                  </div>

                  <div className="flex items-center justify-between rounded border border-border-dark bg-bg-primary p-2.5">
                    <span className="text-text-muted">Xuất bản NLE:</span>
                    <span className="font-semibold text-accent-chat">
                      CapCut Draft + Premiere XML
                    </span>
                  </div>
                </div>
              </div>

              <div className="flex items-center justify-between border-t border-border-dark pt-4">
                <button
                  type="button"
                  onClick={() => setStep(3)}
                  className="text-text-primary inline-flex items-center gap-1.5 rounded-lg border border-border-dark bg-bg-secondary px-3.5 py-2 text-sm font-medium hover:bg-bg-tertiary"
                >
                  <ChevronLeft className="h-4 w-4" />
                  <span>Quay lại</span>
                </button>
                <button
                  type="button"
                  onClick={handleFinish}
                  disabled={isFinishing}
                  className="inline-flex items-center gap-2 rounded-lg bg-accent-vedal px-6 py-2.5 text-sm font-bold text-bg-primary shadow-lg transition-all hover:brightness-110 active:scale-95 disabled:opacity-50"
                >
                  <Sparkles className="h-4 w-4" />
                  <span>{isFinishing ? 'Đang hoàn tất...' : 'Bắt đầu sáng tạo'}</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
