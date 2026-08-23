import {AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';
import {appear, BackgroundPlate, fadeInOut, TopBrand} from './shared';

export const HeroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();

  return (
    <AbsoluteFill
      style={{
        opacity: fadeInOut(frame, durationInFrames),
        backgroundColor: '#03080f',
        overflow: 'hidden',
      }}
    >
      <BackgroundPlate />
      <Img
        src={staticFile('assets/hero-workbench-cinema.webp')}
        style={{
          position: 'absolute',
          right: 0,
          top: 0,
          width: 1120,
          height: 630,
          objectFit: 'cover',
          opacity: interpolate(frame, [0, 28], [0, 0.46], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          scale: interpolate(frame, [0, durationInFrames], [1.04, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          maskImage: 'linear-gradient(90deg, transparent 0%, black 24%, black 74%, transparent 100%)',
        }}
      />
      <TopBrand />
      <div
        style={{
          position: 'absolute',
          left: 130,
          top: 270,
          width: 1050,
          opacity: appear(frame, 18),
          translate: `0px ${interpolate(frame, [18, 52], [42, 0], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          })}px`,
        }}
      >
        <div
          style={{
            color: '#f2c77e',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            fontSize: 26,
            fontWeight: 850,
            letterSpacing: '0.2em',
            marginBottom: 28,
          }}
        >
          {'ASK -> RAMBLE -> CONTINUE'}
        </div>
        <div
          style={{
            color: '#f2f8ff',
            fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
            fontSize: 96,
            fontWeight: 840,
            letterSpacing: 0,
            lineHeight: 1.08,
            textShadow: '0 28px 70px rgb(0 0 0 / 44%)',
          }}
        >
          如何用 ramble
          <br />
          驱动你的开发
        </div>
        <div
          style={{
            marginTop: 34,
            color: 'rgb(214 232 249 / 72%)',
            fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
            fontSize: 34,
            lineHeight: 1.42,
            letterSpacing: 0,
          }}
        >
          When your coding agent needs your eyes.
        </div>
      </div>
      <div
        style={{
          position: 'absolute',
          right: 124,
          bottom: 118,
          display: 'flex',
          alignItems: 'center',
          gap: 18,
          opacity: appear(frame, 48),
        }}
      >
        <Img
          src={staticFile('assets/rambelle-assistant.webp')}
          style={{
            width: 136,
            height: 136,
            objectFit: 'contain',
            filter: 'drop-shadow(0 24px 52px rgb(0 0 0 / 42%))',
          }}
        />
        <div
          style={{
            width: 320,
            padding: '18px 20px',
            border: '1px solid rgb(111 168 220 / 22%)',
            borderRadius: 12,
            background: 'rgb(10 18 31 / 72%)',
            boxShadow: '0 28px 70px rgb(0 0 0 / 30%)',
          }}
        >
          <div
            style={{
              color: '#bffaf6',
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
              fontSize: 17,
              fontWeight: 800,
              letterSpacing: '0.14em',
              marginBottom: 8,
            }}
          >
            LOCAL ARCHIVE ONLINE
          </div>
          <div
            style={{
              color: 'rgb(214 232 249 / 74%)',
              fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
              fontSize: 21,
              lineHeight: 1.35,
            }}
          >
            feedback.md + manifest.json + attachments
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
