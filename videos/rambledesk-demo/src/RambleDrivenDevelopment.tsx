import {AbsoluteFill, Sequence} from 'remotion';
import {AskScene} from './scenes/AskScene';
import {ContinueScene} from './scenes/ContinueScene';
import {HeroScene} from './scenes/HeroScene';
import {RambleScene} from './scenes/RambleScene';
import {SealScene} from './scenes/SealScene';

export const RambleDrivenDevelopment: React.FC = () => {
  return (
    <AbsoluteFill style={{backgroundColor: '#03080f', color: '#f2f8ff'}}>
      <Sequence durationInFrames={120} name="Hero">
        <HeroScene />
      </Sequence>
      <Sequence from={120} durationInFrames={180} name="Ask">
        <AskScene />
      </Sequence>
      <Sequence from={300} durationInFrames={300} name="Ramble">
        <RambleScene />
      </Sequence>
      <Sequence from={600} durationInFrames={120} name="Seal">
        <SealScene />
      </Sequence>
      <Sequence from={720} durationInFrames={120} name="Continue">
        <ContinueScene />
      </Sequence>
    </AbsoluteFill>
  );
};
