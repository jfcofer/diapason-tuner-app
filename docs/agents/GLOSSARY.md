# Glossary

Domain vocabulary. If a term in a doc or a code comment needs explaining, it belongs here — once.

**A4 / reference pitch** — the frequency assigned to the A above middle C, from which every other
target is derived. 440 Hz by convention; historical and orchestral practice ranges 415–466 Hz.

**Cent** — one hundredth of an equal-tempered semitone; `cents = 1200·log₂(f/f_ref)`. The unit the
whole tuner speaks in. Trained ears detect roughly 5 cents; a good tuner resolves under 1.

**Clarity** — the peak value of the NSDF, in [0, 1]. How periodic the signal is, and therefore how
much to trust the estimate. Doubles as the UI's confidence signal.

**Duplex stream** — one audio stream carrying input and output together, sharing a clock. Required
here so the tuner and metronome can run simultaneously without drifting against each other.

**Fundamental (f₀)** — the lowest frequency of a periodic tone; the pitch a listener perceives. Not
necessarily the loudest partial, which is why naive peak-picking on a spectrum fails on guitar.

**Inharmonicity** — real strings have stiffness, so their partials sit slightly above exact integer
multiples of f₀. It is why plucked strings are harder to track than sine waves, and why fixtures
must include a stiff-string model rather than only synthetic tones.

**Lock** — the state where the detected pitch has stayed inside the tolerance band long enough to
be declared in tune. Deliberately hysteretic: the unlock band is wider than the lock band.

**NSDF** — Normalised Square Difference Function, the core of the McLeod Pitch Method: autocorrelation
normalised so its peaks are comparable and bounded, which is what makes clarity meaningful.

**MPM** — McLeod Pitch Method. Peak-picking over the NSDF with parabolic interpolation. Our pitch
algorithm (`adr/0005`).

**Octave error** — reporting a pitch one octave from the true fundamental, the classic failure of
autocorrelation-family detectors and the most visible bug a tuner can have.

**One-euro filter** — an adaptive low-pass whose cutoff rises with the rate of change, giving a
still reading at rest and a responsive one during a peg turn. A fixed filter cannot do both.

**RT thread / audio callback** — the real-time thread the OS calls to fill or drain an audio buffer.
Missing its deadline produces an audible glitch, so allocation, locking and I/O are forbidden there.

**Ring buffer (SPSC)** — single-producer single-consumer lock-free queue; how audio and commands
cross between the RT thread and everything else.

**Snapshot** — the immutable struct the engine publishes for the UI to read at ~30 Hz. Latest-wins,
never queued.

**Strobe** — the display where a rotating pattern's apparent motion is proportional to the pitch
error, so being in tune makes it stand still. Our signature UI element (`DESIGN_SYSTEM.md` §1).

**Subdivision** — clicks between the main beats (eighths, triplets, sixteenths).

**Sweetened tuning** — per-string offsets from equal temperament that make a guitar sound more in
tune with itself across the fretboard, compensating for stretch and intonation.

**Temperament** — the system mapping note names to frequencies. Equal temperament divides the octave
into twelve equal ratios; historical temperaments do not, which is why the app stores cent-offset
tables rather than a formula.

**Xrun** — a buffer underrun or overrun: the audio callback missed its deadline. Counted and shown
in the diagnostics overlay because it is the earliest warning of an RT-safety violation.
