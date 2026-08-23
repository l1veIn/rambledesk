import type {CSSProperties, ReactNode} from 'react';
import {Easing, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';

export const VIDEO_WIDTH = 1920;
export const VIDEO_HEIGHT = 1080;

export const fadeInOut = (frame: number, durationInFrames: number) =>
  interpolate(frame, [0, 18, durationInFrames - 18, durationInFrames], [0, 1, 1, 0], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: Easing.bezier(0.16, 1, 0.3, 1),
  });

export const riseIn = (frame: number, start: number, distance = 34) =>
  interpolate(frame, [start, start + 24], [distance, 0], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: Easing.bezier(0.16, 1, 0.3, 1),
  });

export const appear = (frame: number, start: number) =>
  interpolate(frame, [start, start + 16], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: Easing.bezier(0.16, 1, 0.3, 1),
  });

export const pop = (frame: number, start: number) =>
  interpolate(frame, [start, start + 18], [0.94, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: Easing.bezier(0.16, 1, 0.3, 1),
  });

export const BackgroundPlate: React.FC<{dim?: number}> = ({dim = 1}) => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();

  return (
    <div
      style={{
        position: 'absolute',
        inset: 0,
        overflow: 'hidden',
        background:
          'linear-gradient(180deg, #03080f 0%, #071425 48%, #0a1a2e 74%, #050d18 100%)',
        opacity: dim,
      }}
    >
      <Img
        src={staticFile('assets/rambelle-vault-pattern.webp')}
        style={{
          position: 'absolute',
          width: 760,
          height: 760,
          left: 1020,
          top: 164,
          objectFit: 'cover',
          opacity: 0.18,
          filter: 'invert(1) hue-rotate(180deg) brightness(0.68) contrast(1.04)',
          rotate: `${interpolate(frame, [0, durationInFrames], [-5, 5], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
          })}deg`,
          scale: interpolate(frame, [0, durationInFrames], [1, 1.05], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
          }),
        }}
      />
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background:
            'linear-gradient(90deg, rgb(3 8 15 / 98%) 0%, rgb(3 8 15 / 76%) 42%, rgb(3 8 15 / 26%) 100%)',
        }}
      />
      <div
        style={{
          position: 'absolute',
          left: 90,
          right: 90,
          bottom: 128,
          height: 1,
          background:
            'linear-gradient(90deg, transparent, rgb(87 198 192 / 48%), rgb(242 160 61 / 34%), transparent)',
          boxShadow: '0 0 28px rgb(87 198 192 / 24%)',
        }}
      />
      <div
        style={{
          position: 'absolute',
          inset: 0,
          backgroundImage:
            'linear-gradient(rgb(255 255 255 / 0.035) 1px, transparent 1px), linear-gradient(90deg, rgb(255 255 255 / 0.035) 1px, transparent 1px)',
          backgroundSize: '72px 72px',
          maskImage: 'linear-gradient(180deg, transparent 0%, black 18%, black 72%, transparent 100%)',
        }}
      />
    </div>
  );
};

export const StageLabel: React.FC<{tag: string; title: string; sub: string}> = ({tag, title, sub}) => {
  const frame = useCurrentFrame();

  return (
    <div
      style={{
        position: 'absolute',
        left: 104,
        bottom: 112,
        width: 470,
        opacity: appear(frame, 12),
        translate: `0px ${riseIn(frame, 12, 24)}px`,
      }}
    >
      <div
        style={{
          marginBottom: 16,
          color: '#f2c77e',
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
          fontSize: 22,
          fontWeight: 800,
          letterSpacing: '0.22em',
        }}
      >
        {tag}
      </div>
      <div
        style={{
          color: '#f2f8ff',
          fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          fontSize: 46,
          fontWeight: 780,
          lineHeight: 1.08,
          letterSpacing: 0,
        }}
      >
        {title}
      </div>
      <div
        style={{
          marginTop: 18,
          color: 'rgb(214 232 249 / 68%)',
          fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          fontSize: 25,
          lineHeight: 1.48,
          letterSpacing: 0,
        }}
      >
        {sub}
      </div>
    </div>
  );
};

export const TopBrand: React.FC = () => {
  return (
    <div
      style={{
        position: 'absolute',
        top: 72,
        left: 92,
        display: 'flex',
        alignItems: 'center',
        gap: 16,
      }}
    >
      <Img
        src={staticFile('assets/rambledesk-app-icon.webp')}
        style={{
          width: 54,
          height: 54,
          borderRadius: 14,
          boxShadow: '0 18px 36px rgb(0 0 0 / 30%)',
        }}
      />
      <div
        style={{
          color: '#eaf6ff',
          fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          fontSize: 28,
          fontWeight: 780,
          letterSpacing: 0,
        }}
      >
        RambleDesk
      </div>
    </div>
  );
};

export const TerminalFrame: React.FC<{children: ReactNode; style?: CSSProperties}> = ({children, style}) => {
  return (
    <div
      style={{
        width: 970,
        minHeight: 430,
        padding: 30,
        border: '1px solid rgb(137 169 203 / 18%)',
        borderRadius: 14,
        color: '#e6e6e6',
        background: 'linear-gradient(180deg, rgb(4 8 13 / 96%), rgb(2 5 9 / 98%))',
        boxShadow: '0 50px 140px rgb(0 0 0 / 66%), inset 0 1px 0 rgb(255 255 255 / 5%)',
        fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, "Cascadia Mono", monospace',
        ...style,
      }}
    >
      {children}
    </div>
  );
};

export const TerminalRow: React.FC<{
  children: ReactNode;
  dim?: boolean;
  start?: number;
  accent?: boolean;
}> = ({children, dim = false, start = 0, accent = false}) => {
  const frame = useCurrentFrame();

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'baseline',
        gap: 14,
        minHeight: 40,
        color: accent ? '#d6f5e0' : dim ? 'rgb(217 217 217 / 62%)' : '#d9d9d9',
        fontSize: 26,
        lineHeight: 1.45,
        opacity: appear(frame, start),
        translate: `0px ${riseIn(frame, start, 16)}px`,
      }}
    >
      {children}
    </div>
  );
};

export const PromptArrow: React.FC<{dim?: boolean}> = ({dim = false}) => (
  <span
    style={{
      color: dim ? 'rgb(217 125 108 / 62%)' : '#d97d6c',
      fontWeight: 800,
      fontSize: 30,
      lineHeight: 1,
    }}
  >
    {'>'}
  </span>
);

export const ToolPill: React.FC<{children: ReactNode; start: number}> = ({children, start}) => {
  const frame = useCurrentFrame();

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        width: 'fit-content',
        marginTop: 8,
        padding: '11px 15px',
        border: '1px solid rgb(74 222 128 / 24%)',
        borderRadius: 8,
        color: '#d6f5e0',
        background: 'rgb(74 222 128 / 8%)',
        fontSize: 25,
        fontWeight: 750,
        opacity: appear(frame, start),
        translate: `0px ${riseIn(frame, start, 14)}px`,
        textShadow: '0 0 16px rgb(74 222 128 / 34%)',
      }}
    >
      <span
        style={{
          width: 11,
          height: 11,
          borderRadius: 99,
          background: '#4ade80',
          boxShadow: '0 0 12px rgb(74 222 128 / 80%)',
        }}
      />
      {children}
    </div>
  );
};

export const StepRail: React.FC<{active: number}> = ({active}) => (
  <div
    style={{
      position: 'absolute',
      top: 80,
      right: 94,
      display: 'flex',
      gap: 12,
      alignItems: 'center',
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
      fontSize: 18,
      color: 'rgb(214 232 249 / 62%)',
    }}
  >
    {['ASK', 'RAMBLE', 'CONTINUE'].map((label, index) => (
      <div
        key={label}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 9,
          color: active === index ? '#f2c77e' : 'rgb(214 232 249 / 46%)',
          fontWeight: active === index ? 800 : 650,
          letterSpacing: '0.12em',
        }}
      >
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: 99,
            background: active === index ? '#f2c77e' : 'rgb(214 232 249 / 28%)',
            boxShadow: active === index ? '0 0 14px rgb(242 199 126 / 62%)' : 'none',
          }}
        />
        {label}
      </div>
    ))}
  </div>
);

export const Divider: React.FC = () => (
  <div
    style={{
      height: 1,
      margin: '24px 0',
      background: 'linear-gradient(90deg, rgb(255 255 255 / 22%), rgb(255 255 255 / 8%) 70%, transparent)',
    }}
  />
);
