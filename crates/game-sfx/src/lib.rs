//! Named game sound-effect presets, mixed in software.
//!
//! A single [`wavesynth::engine::SynthEngine`] can't voice more than one
//! [`Instrument`] at a time -- every voice it creates shares the engine's
//! one instrument -- so distinct-sounding concurrent sounds (a fire zap, an
//! explosion boom, a looping thrust hum) each need their own engine.
//! [`GameSynth`] owns one engine per effect and sums their output each
//! sample: real hardware audio mixing on one output almost never exists
//! (confirmed against a real Realtek ALC256's codec node graph -- two DACs,
//! but no mixer on the playback side, only on capture/recording), so
//! summing in software here is the normal, not a workaround.
#![cfg_attr(not(test), no_std)]

use wavesynth::engine::SynthEngine;
use wavesynth::instrument::Instrument;
use wavesynth::midi::MidiEvent;

/// A one-shot game sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Fire,
    Explosion,
}

impl Sfx {
    fn instrument(self) -> Instrument {
        match self {
            Sfx::Fire => Instrument::SquareLead,
            Sfx::Explosion => Instrument::SawBass,
        }
    }

    /// MIDI note number driving the pitch (69 = A4).
    fn note(self) -> u8 {
        match self {
            Sfx::Fire => 84,      // a short, high zap
            Sfx::Explosion => 38, // a low boom
        }
    }

    /// How long to hold the note on before releasing it. Shorter than the
    /// instrument's own release tail isn't required -- the envelope just
    /// keeps fading after this -- it only needs to be short enough that the
    /// sound reads as a "zap"/"boom" rather than a sustained tone.
    fn hold_secs(self) -> f32 {
        match self {
            Sfx::Fire => 0.05,
            Sfx::Explosion => 0.15,
        }
    }
}

const THRUST_INSTRUMENT: Instrument = Instrument::SawBass;
const THRUST_NOTE: u8 = 33; // a low, sustained hum

/// One effect's engine plus the "how much longer to hold the note" timer
/// that turns a single `play()` call into a bounded press-then-release,
/// letting the instrument's own ADSR envelope shape the actual sound.
struct OneShot {
    engine: SynthEngine,
    note: u8,
    hold_samples: u32,
    remaining_samples: u32,
    held: bool,
}

impl OneShot {
    fn new(sample_rate: f32, sfx: Sfx) -> Self {
        Self {
            engine: SynthEngine::new(sample_rate, 1, sfx.instrument()),
            note: sfx.note(),
            hold_samples: (sfx.hold_secs() * sample_rate) as u32,
            remaining_samples: 0,
            held: false,
        }
    }

    fn play(&mut self) {
        self.engine.handle_event(MidiEvent::NoteOn { pitch: self.note, velocity: 127 });
        self.remaining_samples = self.hold_samples;
        self.held = true;
    }

    fn next_sample(&mut self) -> f32 {
        if self.held {
            if self.remaining_samples == 0 {
                self.engine.handle_event(MidiEvent::NoteOff { pitch: self.note });
                self.held = false;
            } else {
                self.remaining_samples -= 1;
            }
        }
        self.engine.next_sample()
    }
}

/// Everything the asteroids game can make noise about, mixed into one mono
/// PCM stream. Callers pull [`GameSynth::next_sample`] continuously (once
/// per output sample) regardless of whether anything is currently
/// sounding -- silence just falls out naturally once every voice's release
/// has finished.
pub struct GameSynth {
    fire: OneShot,
    explosion: OneShot,
    thrust: SynthEngine,
}

impl GameSynth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            fire: OneShot::new(sample_rate, Sfx::Fire),
            explosion: OneShot::new(sample_rate, Sfx::Explosion),
            thrust: SynthEngine::new(sample_rate, 1, THRUST_INSTRUMENT),
        }
    }

    /// Triggers a one-shot sound. Retriggering the same effect while it's
    /// still ringing restarts it -- there's only one voice per effect.
    pub fn play(&mut self, sfx: Sfx) {
        match sfx {
            Sfx::Fire => self.fire.play(),
            Sfx::Explosion => self.explosion.play(),
        }
    }

    pub fn start_thrust(&mut self) {
        self.thrust.handle_event(MidiEvent::NoteOn { pitch: THRUST_NOTE, velocity: 100 });
    }

    pub fn stop_thrust(&mut self) {
        self.thrust.handle_event(MidiEvent::NoteOff { pitch: THRUST_NOTE });
    }

    /// The next combined PCM sample (mono, roughly `[-1.0, 1.0]`).
    pub fn next_sample(&mut self) -> f32 {
        (self.fire.next_sample() + self.explosion.next_sample() + self.thrust.next_sample()) / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn peak_abs(synth: &mut GameSynth, samples: usize) -> f32 {
        (0..samples).map(|_| synth.next_sample().abs()).fold(0.0f32, f32::max)
    }

    #[test]
    fn silent_when_nothing_is_playing() {
        let mut synth = GameSynth::new(SAMPLE_RATE);
        assert_eq!(peak_abs(&mut synth, 1000), 0.0);
    }

    #[test]
    fn fire_produces_sound_and_then_decays_to_silence() {
        let mut synth = GameSynth::new(SAMPLE_RATE);
        synth.play(Sfx::Fire);
        assert!(peak_abs(&mut synth, 100) > 0.01, "fire should be audible right after play()");

        // Run well past the hold time and the instrument's release tail.
        peak_abs(&mut synth, SAMPLE_RATE as usize);
        assert_eq!(peak_abs(&mut synth, 1000), 0.0, "fire should be silent long after it ends");
    }

    #[test]
    fn explosion_produces_sound() {
        let mut synth = GameSynth::new(SAMPLE_RATE);
        synth.play(Sfx::Explosion);
        assert!(peak_abs(&mut synth, 100) > 0.01);
    }

    #[test]
    fn thrust_sustains_until_stopped() {
        let mut synth = GameSynth::new(SAMPLE_RATE);
        synth.start_thrust();
        assert!(peak_abs(&mut synth, (SAMPLE_RATE * 0.5) as usize) > 0.01, "thrust should still be audible after half a second");

        synth.stop_thrust();
        peak_abs(&mut synth, SAMPLE_RATE as usize); // let the release tail finish
        assert_eq!(peak_abs(&mut synth, 1000), 0.0, "thrust should be silent well after stop_thrust()");
    }

    #[test]
    fn fire_and_thrust_mix_together() {
        let mut synth = GameSynth::new(SAMPLE_RATE);
        synth.start_thrust();
        peak_abs(&mut synth, 100);
        synth.play(Sfx::Fire);
        assert!(peak_abs(&mut synth, 100) > 0.01, "both sounds mixed should still be audible");
    }
}
