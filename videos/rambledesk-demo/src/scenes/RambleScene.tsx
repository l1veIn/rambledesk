import {AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';
import {appear, BackgroundPlate, fadeInOut, StageLabel, StepRail} from './shared';

const feed = [
  {kind: 'text', text: 'Pacing: the first scene types too slow...'},
  {kind: 'text', text: 'The CLI window reads right now.'},
  {kind: 'shot', text: 'this block reads wrong'},
  {kind: 'text', text: 'I circled it here.'},
  {kind: 'file', text: 'notes.md'},
];

const context = ['Snapshot', 'Clipboard', 'Files'];

export const RambleScene: React.FC = () => {
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
        tag="RAMBLE"
        title="You ramble, it keeps everything."
        sub="Voice, shots, files - straight into the same draft, no prompt rewriting."
      />
      <div
        style={{
          position: 'absolute',
          left: 580,
          top: 210,
          width: 1080,
          height: 640,
          overflow: 'hidden',
          border: '1px solid rgb(111 168 220 / 30%)',
          borderRadius: 14,
          background: 'linear-gradient(180deg, rgb(17 27 44 / 92%), rgb(10 18 31 / 94%))',
          boxShadow: '0 46px 130px rgb(3 12 24 / 60%), inset 0 1px 0 rgb(255 255 255 / 6%)',
          fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          opacity: appear(frame, 8),
          scale: interpolate(frame, [8, 36], [0.975, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          translate: `0px ${interpolate(frame, [8, 36], [34, 0], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          })}px`,
        }}
      >
        <div
          style={{
            height: 54,
            display: 'flex',
            alignItems: 'center',
            padding: '0 20px',
            borderBottom: '1px solid rgb(111 168 220 / 16%)',
            color: '#cfe3f6',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            fontSize: 17,
            fontWeight: 800,
          }}
        >
          <span>RambleDesk</span>
          <span
            style={{
              marginLeft: 'auto',
              color: 'rgb(184 212 238 / 58%)',
              fontSize: 15,
              fontWeight: 600,
            }}
          >
            DeepSeek Harness · dsh-session-dc2...
          </span>
        </div>
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'minmax(0, 1.35fr) 310px',
            gap: 16,
            padding: 18,
            height: 586,
          }}
        >
          <div style={{display: 'grid', gridTemplateRows: '120px 1fr', gap: 14}}>
            <Panel>
              <div style={{color: '#bffaf6', fontSize: 20, fontWeight: 800}}>Needs your eyes:</div>
              <div style={{marginTop: 12, color: 'rgb(214 232 249 / 84%)', fontSize: 23, lineHeight: 1.38}}>
                Refresh the homepage and scroll below the hero - the loop pacing is off.
              </div>
            </Panel>
            <Panel>
              <div
                style={{
                  color: '#bffaf6',
                  fontSize: 18,
                  fontWeight: 800,
                  letterSpacing: '0.08em',
                  textTransform: 'uppercase',
                  marginBottom: 16,
                }}
              >
                Feedback document
              </div>
              <div style={{display: 'grid', gap: 11}}>
                {feed.map((item, index) => (
                  <div
                    key={item.text}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      minHeight: item.kind === 'shot' ? 78 : 48,
                      gap: 12,
                      padding: item.kind === 'shot' ? '12px 14px' : '9px 13px',
                      border: '1px solid rgb(111 168 220 / 14%)',
                      borderRadius: 10,
                      background: item.kind === 'file' ? 'rgb(242 199 126 / 8%)' : 'rgb(255 255 255 / 4%)',
                      opacity: appear(frame, 58 + index * 20),
                      translate: `0px ${interpolate(frame, [58 + index * 20, 78 + index * 20], [18, 0], {
                        extrapolateLeft: 'clamp',
                        extrapolateRight: 'clamp',
                        easing: Easing.bezier(0.16, 1, 0.3, 1),
                      })}px`,
                    }}
                  >
                    <span
                      style={{
                        width: item.kind === 'shot' ? 68 : 28,
                        height: item.kind === 'shot' ? 48 : 28,
                        borderRadius: item.kind === 'shot' ? 8 : 99,
                        background:
                          item.kind === 'shot'
                            ? 'linear-gradient(135deg, rgb(87 198 192 / 42%), rgb(242 160 61 / 38%))'
                            : item.kind === 'file'
                              ? '#f2c77e'
                              : '#57c6c0',
                        boxShadow: item.kind === 'shot' ? 'inset 0 0 0 2px rgb(255 255 255 / 12%)' : 'none',
                      }}
                    />
                    <span style={{color: 'rgb(230 242 250 / 86%)', fontSize: 21}}>{item.text}</span>
                    {item.kind === 'shot' ? (
                      <span
                        style={{
                          marginLeft: 'auto',
                          color: '#f2c77e',
                          fontSize: 16,
                          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
                          fontWeight: 800,
                        }}
                      >
                        annotate
                      </span>
                    ) : null}
                  </div>
                ))}
              </div>
            </Panel>
          </div>
          <div style={{display: 'grid', gridTemplateRows: '260px 118px 54px 28px', gap: 14}}>
            <Panel>
              <div style={{display: 'flex', alignItems: 'center', gap: 10, color: '#8fd8d2'}}>
                <div style={{fontSize: 23, fontWeight: 800, color: '#d9f4f1'}}>Ramble</div>
                <div
                  style={{
                    marginLeft: 'auto',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 7,
                    minHeight: 28,
                    padding: '0 10px',
                    border: '1px solid rgb(200 107 123 / 42%)',
                    borderRadius: 7,
                    color: '#f3b9c4',
                    background: 'rgb(200 107 123 / 12%)',
                    fontSize: 14,
                    fontWeight: 800,
                    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
                  }}
                >
                  <span
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: 99,
                      background: frame % 36 < 18 ? '#e4687e' : 'rgb(228 104 126 / 35%)',
                    }}
                  />
                  Recording
                </div>
              </div>
              <div
                style={{
                  marginTop: 20,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  height: 42,
                  borderRadius: 9,
                  color: '#d9e8f7',
                  background: 'rgb(255 255 255 / 5%)',
                  fontSize: 19,
                  fontWeight: 750,
                }}
              >
                Pause recording
              </div>
              <div style={{marginTop: 17, color: 'rgb(214 232 249 / 60%)', fontSize: 17}}>
                EDIFIER M30 Plus · 30 clips
              </div>
              <div style={{marginTop: 16, color: 'rgb(214 232 249 / 78%)', fontSize: 19, lineHeight: 1.38}}>
                Listening: “this block reads wrong...”
              </div>
              <div style={{marginTop: 18, display: 'flex', alignItems: 'end', gap: 5, height: 40}}>
                {Array.from({length: 26}).map((_, index) => (
                  <span
                    key={index}
                    style={{
                      display: 'block',
                      width: 6,
                      height: interpolate((frame + index * 5) % 52, [0, 26, 52], [10, 38, 12], {
                        extrapolateLeft: 'clamp',
                        extrapolateRight: 'clamp',
                      }),
                      borderRadius: 99,
                      background: index % 3 === 0 ? '#f2c77e' : '#57c6c0',
                      opacity: 0.78,
                    }}
                  />
                ))}
              </div>
            </Panel>
            <Panel>
              <div style={{fontSize: 18, color: '#bffaf6', fontWeight: 800, marginBottom: 16}}>+ Add context</div>
              <div style={{display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8}}>
                {context.map((item, index) => (
                  <div
                    key={item}
                    style={{
                      display: 'grid',
                      placeItems: 'center',
                      minHeight: 46,
                      borderRadius: 8,
                      background: 'rgb(255 255 255 / 5%)',
                      color: index === 0 ? '#f2c77e' : '#d9e8f7',
                      fontSize: 14,
                      fontWeight: 750,
                    }}
                  >
                    {item}
                  </div>
                ))}
              </div>
            </Panel>
            <button
              type="button"
              style={{
                border: 0,
                borderRadius: 9,
                background: 'linear-gradient(90deg, #57c6c0, #f2a03d)',
                color: '#071425',
                fontSize: 19,
                fontWeight: 850,
                letterSpacing: 0,
                opacity: appear(frame, 178),
              }}
            >
              Submit feedback
            </button>
            <div style={{textAlign: 'center', color: 'rgb(214 232 249 / 48%)', fontSize: 15}}>Cancel feedback</div>
          </div>
        </div>
      </div>
      <Img
        src={staticFile('assets/state-recording.webp')}
        style={{
          position: 'absolute',
          width: 210,
          height: 210,
          objectFit: 'contain',
          right: 108,
          bottom: 80,
          opacity: appear(frame, 102) * 0.86,
          filter: 'drop-shadow(0 24px 58px rgb(0 0 0 / 44%))',
        }}
      />
    </AbsoluteFill>
  );
};

const Panel: React.FC<{children: React.ReactNode}> = ({children}) => {
  return (
    <div
      style={{
        padding: 18,
        border: '1px solid rgb(111 168 220 / 18%)',
        borderRadius: 11,
        background: 'rgb(255 255 255 / 3.5%)',
        overflow: 'hidden',
      }}
    >
      {children}
    </div>
  );
};
