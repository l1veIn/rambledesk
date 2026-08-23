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

const prompt = '/ramble We need to optimize the web page - make it cinematic, nonlinear';
const logs = [
  '* Sprouting... (6s - thinking)',
  'workspace synced - 3 sections below hero scanned',
  'proposal drafted: cinematic nonlinear scroll',
  'human judgment required',
];

export const AskScene: React.FC = () => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  const typed = prompt.slice(0, Math.min(prompt.length, Math.floor(Math.max(0, frame - 28) / 1.45)));

  return (
    <AbsoluteFill
      style={{
        opacity: fadeInOut(frame, durationInFrames),
        backgroundColor: '#03080f',
        overflow: 'hidden',
      }}
    >
      <BackgroundPlate />
      <StepRail active={0} />
      <StageLabel
        tag="ASK"
        title="You type, the agent takes it from there."
        sub="One line in its own CLI - then it syncs, thinks, and knocks."
      />
      <TerminalFrame
        style={{
          position: 'absolute',
          left: 650,
          top: 285,
          opacity: interpolate(frame, [10, 34], [0, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          scale: interpolate(frame, [10, 34], [0.975, 1], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          }),
          translate: `0px ${interpolate(frame, [10, 34], [36, 0], {
            extrapolateLeft: 'clamp',
            extrapolateRight: 'clamp',
            easing: Easing.bezier(0.16, 1, 0.3, 1),
          })}px`,
        }}
      >
        <TerminalRow start={18}>
          <PromptArrow />
          <span style={{color: '#f2f2f2'}}>{typed}</span>
          <span style={{width: 12, height: 25, background: 'rgb(230 230 230 / 82%)'}} />
        </TerminalRow>
        <Divider />
        {logs.map((log, index) => (
          <TerminalRow key={log} start={72 + index * 14} dim={index > 0}>
            <span style={{width: 28, color: index === 3 ? '#f2c77e' : 'rgb(217 217 217 / 54%)'}}>
              {index === 0 ? '*' : index === 3 ? '->' : '✓'}
            </span>
            <span>{log}</span>
          </TerminalRow>
        ))}
        <ToolPill start={130}>request ramble feedback</ToolPill>
        <TerminalRow start={144} dim>
          <span style={{paddingLeft: 24}}>└ waking RambleDesk...</span>
        </TerminalRow>
      </TerminalFrame>
    </AbsoluteFill>
  );
};
