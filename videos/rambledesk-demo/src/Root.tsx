import {Composition, Folder} from 'remotion';
import {RambleDrivenDevelopment} from './RambleDrivenDevelopment';
import {AskScene} from './scenes/AskScene';
import {ContinueScene} from './scenes/ContinueScene';
import {HeroScene} from './scenes/HeroScene';
import {RambleScene} from './scenes/RambleScene';
import {SealScene} from './scenes/SealScene';

export const Root: React.FC = () => {
  return (
    <>
      <Folder name="RambleDrivenDevelopment-Scenes">
        <Composition
          id="HeroScene"
          component={HeroScene}
          durationInFrames={120}
          fps={30}
          width={1920}
          height={1080}
        />
        <Composition
          id="AskScene"
          component={AskScene}
          durationInFrames={180}
          fps={30}
          width={1920}
          height={1080}
        />
        <Composition
          id="RambleScene"
          component={RambleScene}
          durationInFrames={300}
          fps={30}
          width={1920}
          height={1080}
        />
        <Composition
          id="SealScene"
          component={SealScene}
          durationInFrames={120}
          fps={30}
          width={1920}
          height={1080}
        />
        <Composition
          id="ContinueScene"
          component={ContinueScene}
          durationInFrames={120}
          fps={30}
          width={1920}
          height={1080}
        />
      </Folder>
      <Composition
        id="RambleDrivenDevelopment"
        component={RambleDrivenDevelopment}
        durationInFrames={840}
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};
