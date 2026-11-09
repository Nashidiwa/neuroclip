import React, { useState, useEffect } from 'react';
import {
  Sparkles,
  Monitor,
  Activity,
  Layers,
  ShieldCheck,
  Cpu,
  RefreshCw,
  Sliders,
} from 'lucide-react';
import {
  loadTheme,
  resolveBackdropConfig,
  WindowBackdropConfig,
  WINDOW_GEOMETRY,
} from './styles/theme';

export const App: React.FC = () => {
  const theme = loadTheme();
  // Assume acrylic is supported if Windows 11/10 21H2+ is present, otherwise fallback
  const [isAcrylicSupported, setIsAcrylicSupported] = useState<boolean>(true);
  const [backdropConfig, setBackdropConfig] = useState<WindowBackdropConfig>(() =>
    resolveBackdropConfig(true),
  );

  useEffect(() => {
    setBackdropConfig(resolveBackdropConfig(isAcrylicSupported));
  }, [isAcrylicSupported]);

  const handleWindowMinimize = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow().minimize().catch(() => {});
        })
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowMaximize = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow().toggleMaximize().catch(() => {});
        })
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  const handleWindowClose = (): void => {
    try {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => {
          void getCurrentWindow().close().catch(() => {});
        })
        .catch(() => {});
    } catch {
      // Non-tauri browser environment fallback
    }
  };

  return (
    <div
      className={`flex flex-col h-screen w-screen text-slate-100 overflow-hidden ${backdropConfig.activeClassName}`}
      data-testid="app-container"
      style={{
        backgroundColor: isAcrylicSupported ? undefined : backdropConfig.solidFallbackHex,
      }}
    >
      {/* Window TitleBar (Custom Tauri Drag Region) */}
      <header
        data-tauri-drag-region
        className="flex items-center justify-between px-3 h-10 border-b border-border-dark select-none bg-bg-secondary/60 backdrop-blur-md"
      >
        <div className="flex items-center space-x-2 pointer-events-none">
          <div className="w-3 h-3 rounded-full bg-accent-neuro animate-pulse" />
          <span className="font-semibold text-xs tracking-wider text-accent-neuro">
            NEUROCLIP
          </span>
          <span className="text-xs text-slate-400">| AI Subtitle Studio</span>
        </div>

        <div className="text-[11px] font-mono text-slate-400 pointer-events-none">
          Window: {WINDOW_GEOMETRY.defaultWidth}x{WINDOW_GEOMETRY.defaultHeight} (Min: {WINDOW_GEOMETRY.minWidth}x{WINDOW_GEOMETRY.minHeight})
        </div>

        <div className="flex items-center space-x-2">
          {/* Acrylic / Solid Toggle Button */}
          <button
            type="button"
            data-tauri-no-drag
            onClick={() => setIsAcrylicSupported((prev) => !prev)}
            title="Chuyển đổi hiệu ứng Acrylic / Solid Fallback"
            className="px-2 py-0.5 text-[11px] rounded bg-bg-tertiary hover:bg-slate-700 text-slate-300 transition-colors flex items-center space-x-1"
          >
            <Sliders className="w-3 h-3 text-accent-vedal" />
            <span>{isAcrylicSupported ? 'Acrylic: ON' : 'Solid Fallback: #12131C'}</span>
          </button>

          {/* Window Control Buttons */}
          <div className="flex items-center space-x-1 ml-2" data-tauri-no-drag>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowMinimize}
              title="Thu nhỏ cửa sổ"
              className="w-7 h-6 rounded flex items-center justify-center text-slate-400 hover:text-white hover:bg-bg-tertiary transition-colors text-xs font-mono"
            >
              &#x2014;
            </button>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowMaximize}
              title="Phóng to / Khôi phục cửa sổ"
              className="w-7 h-6 rounded flex items-center justify-center text-slate-400 hover:text-white hover:bg-bg-tertiary transition-colors text-xs font-mono"
            >
              &#x25A2;
            </button>
            <button
              type="button"
              data-tauri-no-drag
              onClick={handleWindowClose}
              title="Đóng ứng dụng"
              className="w-7 h-6 rounded flex items-center justify-center text-slate-400 hover:text-white hover:bg-accent-evil transition-colors text-xs font-mono"
            >
              &#x2715;
            </button>
          </div>
        </div>
      </header>

      {/* Main Workspace Body */}
      <main className="flex-1 flex flex-col p-6 space-y-6 overflow-y-auto">
        {/* Hero Section */}
        <div className="flex items-center justify-between bg-bg-secondary/70 border border-border-dark p-6 rounded-xl shadow-acrylic">
          <div className="space-y-2">
            <div className="flex items-center space-x-2">
              <Sparkles className="w-6 h-6 text-accent-neuro" />
              <h1 className="text-xl font-bold tracking-tight text-white">
                Tauri v2 Native Desktop Shell
              </h1>
            </div>
            <p className="text-sm text-slate-300 max-w-2xl leading-relaxed">
              Khung ứng dụng máy tính đa nền tảng tối ưu hóa cho Windows 10/11. Hệ thống giao diện
              Acrylic Dark Theme phong cách VTuber Neuro-sama, tích hợp font chữ Be Vietnam Pro và
              Inter tối ưu dấu tiếng Việt.
            </p>
          </div>

          <div className="flex flex-col items-end space-y-1 text-right">
            <span className="text-xs font-mono text-slate-400">Milestone Foundation</span>
            <span className="px-2.5 py-1 text-xs font-semibold rounded-full bg-accent-neuro/20 text-accent-neuro border border-accent-neuro/40">
              TASK-M4-01 Verified
            </span>
          </div>
        </div>

        {/* VTuber Neuro-sama Character Palette Badges */}
        <section className="space-y-3">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-slate-400 flex items-center space-x-1.5">
            <Layers className="w-4 h-4 text-accent-neuro" />
            <span>Bảng Màu Nhân Vật VTuber & Giao Diện (Neuro-sama Lore Palette)</span>
          </h2>

          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3">
            <div className="p-3 rounded-lg bg-bg-secondary border border-border-dark flex items-center space-x-3">
              <div className="w-5 h-5 rounded-md shadow-sm" style={{ backgroundColor: theme.accentNeuro }} />
              <div>
                <p className="text-xs font-semibold text-white">Neuro-sama</p>
                <p className="text-[10px] font-mono text-slate-400">{theme.accentNeuro}</p>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-bg-secondary border border-border-dark flex items-center space-x-3">
              <div className="w-5 h-5 rounded-md shadow-sm" style={{ backgroundColor: theme.accentEvil }} />
              <div>
                <p className="text-xs font-semibold text-white">Evil Neuro</p>
                <p className="text-[10px] font-mono text-slate-400">{theme.accentEvil}</p>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-bg-secondary border border-border-dark flex items-center space-x-3">
              <div className="w-5 h-5 rounded-md shadow-sm" style={{ backgroundColor: theme.accentVedal }} />
              <div>
                <p className="text-xs font-semibold text-white">Vedal (Tutel)</p>
                <p className="text-[10px] font-mono text-slate-400">{theme.accentVedal}</p>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-bg-secondary border border-border-dark flex items-center space-x-3">
              <div className="w-5 h-5 rounded-md shadow-sm" style={{ backgroundColor: theme.accentGuest }} />
              <div>
                <p className="text-xs font-semibold text-white">Khách mời (Gold)</p>
                <p className="text-[10px] font-mono text-slate-400">{theme.accentGuest}</p>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-bg-secondary border border-border-dark flex items-center space-x-3">
              <div className="w-5 h-5 rounded-md shadow-sm" style={{ backgroundColor: theme.accentChat }} />
              <div>
                <p className="text-xs font-semibold text-white">Chat / Swarm</p>
                <p className="text-[10px] font-mono text-slate-400">{theme.accentChat}</p>
              </div>
            </div>
          </div>
        </section>

        {/* SLA and Architecture Metrics */}
        <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="p-4 rounded-xl bg-bg-secondary border border-border-dark space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-slate-400">Static Idle RAM</span>
              <Cpu className="w-4 h-4 text-accent-vedal" />
            </div>
            <div className="text-2xl font-bold text-white">≤ 90 MB</div>
            <p className="text-[11px] text-slate-400 leading-snug">
              Kiểm soát bộ nhớ nghiêm ngặt qua Windows Task Manager khi chưa nạp video.
            </p>
          </div>

          <div className="p-4 rounded-xl bg-bg-secondary border border-border-dark space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-slate-400">Khởi Động Nhanh (TTI)</span>
              <Activity className="w-4 h-4 text-accent-neuro" />
            </div>
            <div className="text-2xl font-bold text-white">≤ 800 ms</div>
            <p className="text-[11px] text-slate-400 leading-snug">
              Thời gian sẵn sàng tương tác trên WebView2 Windows 10/11.
            </p>
          </div>

          <div className="p-4 rounded-xl bg-bg-secondary border border-border-dark space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-slate-400">Độ Mượt Render</span>
              <Monitor className="w-4 h-4 text-accent-chat" />
            </div>
            <div className="text-2xl font-bold text-white">60 FPS</div>
            <p className="text-[11px] text-slate-400 leading-snug">
              Duy trì ≥ 55 FPS ổn định khi kéo giãn thay đổi kích thước cửa sổ.
            </p>
          </div>
        </section>

        {/* Architectural Directives Checklist */}
        <section className="p-5 rounded-xl bg-bg-secondary/50 border border-border-dark space-y-3">
          <div className="flex items-center space-x-2">
            <ShieldCheck className="w-5 h-5 text-accent-vedal" />
            <h3 className="text-sm font-semibold text-white">
              Tiêu Chuẩn Kỹ Thuật Foundation Bedrock
            </h3>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-xs text-slate-300">
            <div className="flex items-center space-x-2">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-vedal" />
              <span>Rust Backend: Edition 2021, target x86_64-pc-windows-msvc</span>
            </div>
            <div className="flex items-center space-x-2">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-vedal" />
              <span>Clippy Zero-Panic: #![deny(clippy::unwrap_used, clippy::panic)]</span>
            </div>
            <div className="flex items-center space-x-2">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-vedal" />
              <span>Process Supervision: Windows Job Object Kill-on-Drop</span>
            </div>
            <div className="flex items-center space-x-2">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-vedal" />
              <span>TypeScript Strict: noImplicitAny, exactOptionalPropertyTypes</span>
            </div>
          </div>
        </section>
      </main>

      {/* Bottom Status Bar */}
      <footer className="h-7 border-t border-border-dark px-4 flex items-center justify-between text-[11px] font-mono text-slate-400 bg-bg-secondary select-none">
        <div className="flex items-center space-x-4">
          <span className="flex items-center space-x-1.5">
            <span className="w-2 h-2 rounded-full bg-accent-vedal" />
            <span>Tauri v2 Desktop Shell Ready</span>
          </span>
          <span>|</span>
          <span>SLA Idle RAM: &lt;90MB</span>
          <span>|</span>
          <span>Theme: {isAcrylicSupported ? 'Acrylic / Mica' : 'Solid Fallback #12131C'}</span>
        </div>

        <div className="flex items-center space-x-3">
          <span className="text-accent-neuro font-semibold">NeuroClip Studio v0.1.0</span>
          <RefreshCw className="w-3 h-3 text-slate-500 animate-spin" style={{ animationDuration: '8s' }} />
        </div>
      </footer>
    </div>
  );
};

export default App;
