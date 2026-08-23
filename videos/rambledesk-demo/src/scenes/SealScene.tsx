import {AbsoluteFill, Easing, interpolate, useCurrentFrame, useVideoConfig} from 'remotion';
import {appear, BackgroundPlate, fadeInOut, pop, StageLabel, StepRail} from './shared';

const files = ['feedback.md', 'uncooked.md', 'manifest.json', 'attachments/'];

export const SealScene: React.FC = () => {
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
      <StepRail active={1} />
      <StageLabel
        tag="PACKAGE"
        title="Evidence gets sealed."
        sub="The ramble becomes a portable package, not another chat message."
      />
      <div
        style={{
          position: 'absolute',
          left: 660,
          top: 220,
          width: 850,
          minHeight: 560,
          padding: 42,
          border: '1px solid rgb(111 168 220 / 28%)',
          borderRadius: 18,
          background: 'linear-gradient(180deg, rgb(17 27 44 / 94%), rgb(7 16 28 / 96%))',
          boxShadow: '0 52px 150px rgb(0 0 0 / 58%), inset 0 1px 0 rgb(255 255 255 / 6%)',
          opacity: appear(frame, 10),
          scale: pop(frame, 10),
        }}
      >
        <div
          style={{
            color: '#f2f8ff',
            fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
            fontSize: 54,
            fontWeight: 820,
            letterSpacing: 0,
            lineHeight: 1.08,
          }}
        >
          Sealing feedback package
        </div>
        <div style={{display: 'grid', gap: 14, marginTop: 34}}>
          {files.map((file, index) => (
            <div
              key={file}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 16,
                minHeight: 58,
                padding: '0 18px',
                border: '1px solid rgb(111 168 220 / 18%)',
                borderRadius: 10,
                background: index === 2 ? 'rgb(87 198 192 / 8%)' : 'rgb(255 255 255 / 4%)',
                opacity: appear(frame, 34 + index * 10),
                translate: `0px ${interpolate(frame, [34 + index * 10, 52 + index * 10], [16, 0], {
                  extrapolateLeft: 'clamp',
                  extrapolateRight: 'clamp',
                  easing: Easing.bezier(0.16, 1, 0.3, 1),
                })}px`,
              }}
            >
              <span
                style={{
                  width: 18,
                  height: 18,
                  borderRadius: 99,
                  background: '#4ade80',
                  boxShadow: '0 0 14px rgb(74 222 128 / 74%)',
                }}
              />
              <span
                style={{
                  color: '#dbeefa',
                  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
                  fontSize: 27,
                  fontWeight: 750,
                }}
              >
                {file}
              </span>
            </div>
          ))}
        </div>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            marginTop: 34,
            padding: '17px 20px',
            borderRadius: 11,
            background: 'rgb(242 199 126 / 9%)',
            color: '#f2c77e',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            fontSize: 24,
            fontWeight: 800,
            opacity: appear(frame, 80),
          }}
        >
          <span>sha256:7c1f4e...3a9ae2</span>
          <span>LOCAL</span>
        </div>
        <div
          style={{
            position: 'absolute',
            right: -54,
            top: -54,
            display: 'grid',
            placeItems: 'center',
            width: 142,
            height: 142,
            borderRadius: 99,
            background: 'linear-gradient(180deg, #57c6c0, #4ade80)',
            color: '#071425',
            fontSize: 82,
            fontWeight: 900,
            boxShadow: '0 28px 80px rgb(74 222 128 / 38%)',
            opacity: appear(frame, 94),
            scale: interpolate(frame, [94, 112], [1.42, 1], {
              extrapolateLeft: 'clamp',
              extrapolateRight: 'clamp',
              easing: Easing.bezier(0.16, 1, 0.3, 1),
            }),
          }}
        >
          ✓
        </div>
      </div>
    </AbsoluteFill>
  );
};
