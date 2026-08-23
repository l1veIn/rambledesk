import {AbsoluteFill, Easing, interpolate, useCurrentFrame, useVideoConfig} from 'remotion';
import {
  BackgroundPlate,
  Divider,
  fadeInOut,
  PromptArrow,
  StageLabel,
  StepRail,
  TerminalFrame,
  TerminalRow,
  ToolPill,
} from './shared';

const oldPrompt = '/ramble We need to optimize the web page - make it cinematic, nonlinear';
const prompt = 'get ramble feedback';
const lines = ['feedback package received', 'resuming implementation with human evidence...'];

export const ContinueScene: React.FC = () => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const typed = prompt.slice(0, Math.min(prompt.length, Math.floor(Math.max(0, frame - 30) / 1.2)));

  return (
    <AbsoluteFill
      style={{
        opacity: fadeInOut(frame, durationInFrames),
        backgroundColor: '#03080f',
        overflow: 'hidden',
      }}
    >
      <BackgroundPlate />
      <StepRail active={2} />
      <StageLabel
        tag="CONTINUE"
        title="The agent reads it - and keeps going."
        sub="One hash, zero bytes leaving your machine."
      />
      <TerminalFrame
        style={{
          position: 'absolute',
          left: 650,
          top: 260,
          opacity: interpolate(frame, [8, 30], [0, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          translate: `0px ${interpolate(frame, [8, 30], [34, 0], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          })}px`,
        }}
      >
        <TerminalRow start={10} dim>
          <PromptArrow dim />
          <span>{oldPrompt}</span>
        </TerminalRow>
        <ToolPill start={20}>request ramble feedback</ToolPill>
        <TerminalRow start={30}>
          <PromptArrow />
          <span style={{color: '#f2f2f2'}}>{typed}</span>
          <span style={{width: 12, height: 25, background: 'rgb(230 230 230 / 82%)'}} />
        </TerminalRow>
        <ToolPill start={56}>get ramble feedback</ToolPill>
        {lines.map((line, index) => (
          <TerminalRow key={line} start={72 + index * 16}>
            <span style={{width: 28, color: '#4ade80'}}>✓</span>
            <span>{line}</span>
          </TerminalRow>
        ))}
        <Divider />
        <TerminalRow start={96}>
          <PromptArrow dim />
          <span style={{width: 14, height: 28, background: 'rgb(230 230 230 / 82%)'}} />
        </TerminalRow>
      </TerminalFrame>
      <div
        style={{
          position: 'absolute',
          left: 650,
          right: 260,
          bottom: 92,
          textAlign: 'center',
          opacity: interpolate(frame, [84, 108], [0, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
        }}
      >
        <div
          style={{
            color: '#f2c77e',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
            fontSize: 23,
            fontWeight: 850,
            letterSpacing: '0.22em',
            textTransform: 'uppercase',
          }}
        >
          THE LOOP, IN ONE TAKE
        </div>
      </div>
    </AbsoluteFill>
  );
};
