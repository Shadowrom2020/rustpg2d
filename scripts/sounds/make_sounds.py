"""Ambience loops, footsteps, UI sounds and spell sounds for R(ust)PG2D's example and starter
library (synthesized, mono 44.1 kHz). Everything here is made for R(ust)PG2D (MIT License, like
the example project).

Usage: python3 scripts/sounds/make_sounds.py OUT_DIR [NAME...] (numpy; ffmpeg with libvorbis for
the loops). Without names it writes everything; the same sources give the same files.

Loops are written as OGG Vorbis (ffmpeg), the rest as WAV. Loops end by crossfading their tail
into their head (equal power), so they repeat without a seam; the engine stretches them to whole
mixer buffers (gapless) when they play.
"""
import os
import subprocess
import sys
import wave

import numpy as np

RATE = 44100
rng = np.random.default_rng(7)
OUT = sys.argv[1]


def lowpass(x, cutoff):
    """One-pole low-pass (cutoff in Hz)."""
    a = np.exp(-2 * np.pi * cutoff / RATE)
    y = np.empty_like(x)
    acc = 0.0
    for i, v in enumerate(x):
        acc = (1 - a) * v + a * acc
        y[i] = acc
    return y


def highpass(x, cutoff):
    return x - lowpass(x, cutoff)


def bandpass(x, f, q):
    """RBJ biquad band-pass (constant 0 dB peak); `f` may be an array (a moving centre)."""
    f = np.broadcast_to(np.asarray(f, dtype=float), x.shape)
    y = np.zeros_like(x)
    x1 = x2 = y1 = y2 = 0.0
    for i, v in enumerate(x):
        w = 2 * np.pi * f[i] / RATE
        alpha = np.sin(w) / (2 * q)
        b0, b2 = alpha, -alpha
        a0, a1, a2 = 1 + alpha, -2 * np.cos(w), 1 - alpha
        out = (b0 * v + b2 * x2 - a1 * y1 - a2 * y2) / a0
        x2, x1, y2, y1 = x1, v, y1, out
        y[i] = out
    return y


def env(n, attack, decay):
    """Attack (s) then exponential decay (time constant s), `n` samples."""
    t = np.arange(n) / RATE
    return np.minimum(t / max(attack, 1e-4), 1.0) * np.exp(-np.maximum(t - attack, 0) / decay)


def place(buf, sig, at):
    at = int(at)
    end = min(len(buf), at + len(sig))
    if end > at:
        buf[at:end] += sig[: end - at]


def seamless(sig, fade_s=1.5):
    """The last `fade_s` seconds faded into the start (equal power); the result is shorter."""
    f = int(fade_s * RATE)
    body = sig[:-f].copy()
    t = np.linspace(0, np.pi / 2, f)
    body[:f] = body[:f] * np.sin(t) + sig[-f:] * np.cos(t)
    return body


def norm(sig, peak):
    return sig / max(np.abs(sig).max(), 1e-9) * peak


def write_wav(path, sig):
    data = (np.clip(sig, -1, 1) * 32767).astype("<i2").tobytes()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(data)


def write_ogg(path, sig):
    tmp = path + ".tmp.wav"
    write_wav(tmp, sig)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", tmp, "-c:a", "libvorbis", "-q:a", "3", path], check=True)
    os.remove(tmp)


def chirp(f0, f1, dur, harm=0.25):
    n = int(dur * RATE)
    t = np.arange(n) / RATE
    f = np.linspace(f0, f1, n)
    ph = 2 * np.pi * np.cumsum(f) / RATE
    s = np.sin(ph) + harm * np.sin(2 * ph)
    return s * np.sin(np.pi * np.minimum(t / dur, 1.0)) ** 2


# ---- loops -----------------------------------------------------------------------------

def birds(seconds=20):
    n = int(seconds * RATE)
    breeze = lowpass(highpass(rng.standard_normal(n), 300), 1800)
    breeze *= 0.5 + 0.5 * np.sin(2 * np.pi * np.arange(n) / RATE / 7.0) ** 2
    out = norm(breeze, 0.05)
    t = 0.3
    while t < seconds - 2.5:
        kind = rng.integers(0, 4)
        base = rng.uniform(2600, 4200)
        song = np.zeros(int(1.6 * RATE))
        if kind == 0:  # trill: quick up-down notes
            for k in range(rng.integers(5, 9)):
                place(song, chirp(base, base * 1.25, 0.05), k * 0.065 * RATE)
        elif kind == 1:  # two-note call
            place(song, chirp(base * 1.2, base * 1.15, 0.16), 0)
            place(song, chirp(base * 0.9, base * 0.85, 0.2), 0.22 * RATE)
        elif kind == 2:  # rising whistle
            place(song, chirp(base * 0.7, base * 1.3, 0.35, harm=0.1), 0)
        else:  # chatter
            for k in range(rng.integers(3, 6)):
                f = base * rng.uniform(0.8, 1.3)
                place(song, chirp(f, f * rng.uniform(0.85, 1.15), rng.uniform(0.03, 0.08)), k * rng.uniform(0.07, 0.12) * RATE)
        place(out, song * rng.uniform(0.12, 0.3), t * RATE)
        t += rng.uniform(0.5, 1.8)
    return norm(seamless(out), 0.7)


def river(seconds=16):
    n = int(seconds * RATE)
    w = rng.standard_normal(n)
    body = bandpass(w, 700, 0.5) * 0.8 + bandpass(w, 2400, 0.7) * 0.35
    body *= 1 + 0.15 * np.sin(2 * np.pi * np.arange(n) / RATE * 0.37)
    out = norm(body, 0.5)
    for _ in range(int(seconds * 9)):  # gurgles: tiny resonant bubbles
        f = rng.uniform(500, 1400)
        d = rng.uniform(0.03, 0.08)
        b = chirp(f, f * rng.uniform(1.2, 1.8), d, harm=0) * rng.uniform(0.05, 0.18)
        place(out, b, rng.uniform(0, seconds - 0.2) * RATE)
    return norm(seamless(out), 0.7)


def tavern(seconds=18):
    n = int(seconds * RATE)
    out = np.zeros(n)
    t = np.arange(n) / RATE
    for v in range(7):  # voices: noise through a moving formant, syllable rhythm
        w = rng.standard_normal(n)
        f = rng.uniform(350, 900) + 180 * np.sin(2 * np.pi * t * rng.uniform(0.6, 1.4) + rng.uniform(0, 6))
        voice = bandpass(w, f, 4.0)
        syll = np.clip(np.sin(2 * np.pi * t * rng.uniform(3, 5.5) + rng.uniform(0, 6)), 0, None) ** 1.5
        phrase = (np.sin(2 * np.pi * t / rng.uniform(2.5, 5) + rng.uniform(0, 6)) > -0.2).astype(float)
        out += voice * syll * lowpass(phrase, 3) * rng.uniform(0.6, 1.0)
    out = norm(lowpass(out, 2500), 0.45)
    hearth = lowpass(rng.standard_normal(n), 400) * 0.05
    out += hearth
    for _ in range(int(seconds / 2.2)):  # mugs and cutlery
        f = rng.uniform(1800, 3200)
        d = int(0.25 * RATE)
        ping = (np.sin(2 * np.pi * f * np.arange(d) / RATE) + 0.5 * np.sin(2 * np.pi * f * 2.7 * np.arange(d) / RATE)) * env(d, 0.001, 0.05)
        place(out, ping * rng.uniform(0.08, 0.2), rng.uniform(0, seconds - 0.3) * RATE)
    for _ in range(int(seconds / 6)):  # a laugh: a few bursts
        at = rng.uniform(0, seconds - 1)
        for k in range(rng.integers(3, 6)):
            d = int(0.09 * RATE)
            burst = bandpass(rng.standard_normal(d), rng.uniform(600, 900), 3) * env(d, 0.01, 0.04)
            place(out, norm(burst, 0.25), (at + k * 0.13) * RATE)
    return norm(seamless(out), 0.7)


def wind(seconds=18):
    n = int(seconds * RATE)
    t = np.arange(n) / RATE
    gust = 0.55 + 0.45 * np.sin(2 * np.pi * t / 6.0) * np.sin(2 * np.pi * t / 2.3 + 1.0)
    f = 380 + 260 * gust
    out = bandpass(rng.standard_normal(n), f, 1.6) * gust
    out += lowpass(rng.standard_normal(n), 250) * 0.4 * gust
    return norm(seamless(out, 2.0), 0.7)


def drips(seconds=16):
    n = int(seconds * RATE)
    out = lowpass(rng.standard_normal(n), 120) * 0.25
    out = norm(out, 0.08)
    for _ in range(int(seconds * 1.4)):
        f = rng.uniform(900, 1700)
        d = int(0.5 * RATE)
        tt = np.arange(d) / RATE
        plink = np.sin(2 * np.pi * (f * tt - 900 * tt * tt)) * env(d, 0.001, 0.05)
        at = rng.uniform(0, seconds - 1) * RATE
        g = rng.uniform(0.15, 0.4)
        for k, (dt, a) in enumerate([(0, 1.0), (0.11, 0.35), (0.23, 0.15), (0.37, 0.06)]):  # the cave's echo
            place(out, plink * g * a, at + dt * RATE)
    return norm(seamless(out), 0.7)


# ---- steps -----------------------------------------------------------------------------

def step(kind):
    """A soft footstep: a short muffled thump with the ground's texture on top (no sharp clicks)."""
    d = int(0.26 * RATE)
    w = rng.standard_normal(d)
    t = np.arange(d) / RATE
    thump = lowpass(lowpass(w, 220), 220) * env(d, 0.008, 0.035)
    if kind == "grass":
        s = 0.5 * thump + bandpass(w, 2200, 0.8) * env(d, 0.02, 0.05) * 0.5
    elif kind == "dirt":
        s = thump + 0.25 * bandpass(w, 1200, 0.9) * env(d, 0.01, 0.03)
    elif kind == "stone":
        s = 0.8 * thump + 0.35 * bandpass(w, 1800, 1.5) * env(d, 0.003, 0.02)
    elif kind == "wood":
        s = 0.5 * thump + 0.5 * (np.sin(2 * np.pi * 150 * t) + 0.4 * np.sin(2 * np.pi * 330 * t)) * env(d, 0.004, 0.04)
    elif kind == "snow":
        s = 0.5 * thump
        for k in range(10):  # a soft crunch: grains packing down
            g = int(rng.uniform(0.01, 0.1) * RATE)
            gd = int(0.015 * RATE)
            place(s, bandpass(rng.standard_normal(gd), 2600, 1.2) * env(gd, 0.002, 0.004) * rng.uniform(0.3, 0.7), g)
    s = lowpass(s, 5000)
    return norm(s, 0.55)


# ---- 0.33: more steps, UI and magic (their own random stream: the sounds above stay as they are)

fx = np.random.default_rng(33)


def noise(n):
    return fx.standard_normal(n)


def tone(f, dur, harm=(1.0,), decay=0.2, attack=0.002):
    """A sine (with harmonics `harm`: amplitudes of 1x, 2x, 3x...) under an attack-decay envelope."""
    n = int(dur * RATE)
    t = np.arange(n) / RATE
    f = np.broadcast_to(np.asarray(f, dtype=float), (n,)) if np.ndim(f) else np.full(n, float(f))
    ph = 2 * np.pi * np.cumsum(f) / RATE
    s = sum(a * np.sin((k + 1) * ph) for k, a in enumerate(harm))
    return s * env(n, attack, decay)


def step2(kind):
    """Footsteps on the newer grounds: sand, gravel, shallow water, mud, ice."""
    d = int(0.3 * RATE)
    w = noise(d)
    thump = lowpass(lowpass(w, 200), 200) * env(d, 0.008, 0.035)
    if kind == "sand":
        s = 0.5 * thump + 0.35 * lowpass(highpass(w, 900), 3500) * env(d, 0.03, 0.07)
    elif kind == "gravel":
        s = 0.6 * thump
        for _ in range(16):  # stones knocking
            g = int(fx.uniform(0.0, 0.09) * RATE)
            gd = int(0.012 * RATE)
            place(s, bandpass(noise(gd), fx.uniform(1500, 3200), 2.5) * env(gd, 0.001, 0.003) * fx.uniform(0.4, 1.0), g)
    elif kind == "water":
        s = 0.3 * thump + 0.5 * bandpass(w, 900, 0.7) * env(d, 0.01, 0.06)
        for _ in range(5):  # drops falling back
            f = fx.uniform(700, 1500)
            place(s, chirp(f, f * 1.6, 0.04, harm=0) * fx.uniform(0.15, 0.35), int(fx.uniform(0.04, 0.2) * RATE))
    elif kind == "mud":
        s = 0.8 * thump + 0.5 * lowpass(w, 700) * env(d, 0.04, 0.08)
        place(s, chirp(260, 520, 0.08, harm=0.3) * 0.35, int(0.09 * RATE))  # the squelch as the foot lifts
    elif kind == "ice":
        s = 0.5 * thump + 0.4 * bandpass(w, 3500, 3) * env(d, 0.001, 0.008)
        place(s, tone(2100, 0.12, (1.0, 0.3), 0.03) * 0.15, int(0.005 * RATE))
    return norm(lowpass(s, 6000), 0.55)


def ui(kind):
    """Short sounds for menus and windows."""
    if kind == "ui_move":
        s = tone(1400, 0.04, (1.0, 0.2), 0.012)
        return norm(s, 0.35)
    if kind == "ui_confirm":
        s = np.zeros(int(0.16 * RATE))
        place(s, tone(880, 0.08, (1.0, 0.3, 0.1), 0.04), 0)
        place(s, tone(1320, 0.1, (1.0, 0.3, 0.1), 0.05), int(0.055 * RATE))
        return norm(s, 0.45)
    if kind == "ui_back":
        s = np.zeros(int(0.16 * RATE))
        place(s, tone(990, 0.08, (1.0, 0.3), 0.04), 0)
        place(s, tone(660, 0.1, (1.0, 0.3), 0.05), int(0.055 * RATE))
        return norm(s, 0.4)
    if kind == "ui_error":
        s = np.zeros(int(0.24 * RATE))
        for k in range(2):
            place(s, tone(185, 0.08, (1.0, 0.6, 0.4, 0.3), 0.05), int(k * 0.1 * RATE))
        return norm(lowpass(s, 2500), 0.45)
    if kind == "coin":
        s = np.zeros(int(0.5 * RATE))
        for k, f in enumerate([2400, 3150, 2800]):  # pieces landing on each other
            place(s, tone(f, 0.35, (1.0, 0.0, 0.4, 0.0, 0.2), 0.07) * (1.0 - 0.25 * k), int(k * 0.06 * RATE))
        return norm(s, 0.45)
    if kind == "quest_done":
        s = np.zeros(int(1.4 * RATE))
        for k, f in enumerate([523.25, 659.25, 783.99, 1046.5]):  # C E G C
            place(s, tone(f, 1.1, (1.0, 0.4, 0.15), 0.35, 0.004) * (0.8 if k < 3 else 1.0), int(k * 0.09 * RATE))
        return norm(s, 0.5)
    if kind == "page":
        n = int(0.35 * RATE)
        t = np.arange(n) / RATE
        s = bandpass(noise(n), 2500 + 2000 * t / 0.35, 1.2) * np.sin(np.pi * t / 0.35) ** 2
        return norm(s, 0.35)
    raise SystemExit(f"no UI sound {kind}")


def magic(kind):
    """Spell sounds by kind of magic."""
    if kind == "magic_frost":
        n = int(1.1 * RATE)
        s = 0.3 * bandpass(noise(n), 5000, 1.0) * env(n, 0.05, 0.25)
        for _ in range(14):  # ice crystals forming
            f = fx.uniform(2500, 5500)
            place(s, tone(f, 0.4, (1.0, 0.2), 0.12) * fx.uniform(0.1, 0.3), int(fx.uniform(0, 0.6) * RATE))
        return norm(s, 0.5)
    if kind == "magic_thunder":
        n = int(1.6 * RATE)
        crack = highpass(noise(n), 1500) * env(n, 0.001, 0.03)
        rumble = lowpass(lowpass(noise(n), 120), 120) * env(n, 0.05, 0.5)
        return norm(crack * 0.8 + norm(rumble, 1.0) * 0.9, 0.7)
    if kind == "magic_heal":
        s = np.zeros(int(1.4 * RATE))
        for k, f in enumerate([392.0, 493.88, 587.33, 783.99]):  # a G chord blooming upward
            place(s, tone(f, 1.2, (1.0, 0.3), 0.5, 0.15) * 0.6, int(k * 0.07 * RATE))
        for _ in range(10):  # sparkles
            f = fx.uniform(2500, 4500)
            place(s, tone(f, 0.25, (1.0,), 0.06) * fx.uniform(0.08, 0.18), int(fx.uniform(0.1, 0.9) * RATE))
        return norm(s, 0.5)
    if kind == "magic_dark":
        n = int(1.4 * RATE)
        t = np.arange(n) / RATE
        f = 110 * np.exp(-t * 0.6)
        s = tone(f, 1.4, (1.0, 0.5, 0.3), 0.6, 0.2) + tone(f * 1.06, 1.4, (1.0, 0.5), 0.6, 0.2)
        s = lowpass(s, 900) + 0.25 * bandpass(noise(n), 700 + 300 * np.sin(2 * np.pi * t * 3), 4) * env(n, 0.2, 0.5)  # whispers
        return norm(s, 0.55)
    if kind == "magic_force":
        s = np.zeros(int(0.6 * RATE))
        for k in range(3):  # three darts
            place(s, chirp(2400, 500, 0.12, harm=0.4), int(k * 0.12 * RATE))
        return norm(s, 0.45)
    if kind == "magic_charm":
        n = int(1.2 * RATE)
        t = np.arange(n) / RATE
        vib = 1 + 0.02 * np.sin(2 * np.pi * 6 * t)
        s = tone(880 * vib, 1.2, (1.0, 0.2), 0.5, 0.2) + tone(1479.98 * vib, 1.2, (1.0,), 0.5, 0.25) * 0.6
        return norm(s, 0.45)
    if kind == "magic_acid":
        n = int(1.0 * RATE)
        s = 0.4 * highpass(noise(n), 3000) * env(n, 0.02, 0.35)
        for _ in range(30):  # bubbles popping
            f = fx.uniform(400, 1200)
            place(s, chirp(f, f * 2.2, 0.03, harm=0) * fx.uniform(0.1, 0.3), int(fx.uniform(0, 0.8) * RATE))
        return norm(s, 0.5)
    if kind == "magic_ward":
        n = int(1.3 * RATE)
        s = tone(220, 1.3, (1.0, 0.5, 0.33, 0.25), 0.45, 0.25) * 0.5
        place(s, tone(1320, 1.0, (1.0, 0.0, 0.3), 0.4) * 0.35, int(0.12 * RATE))
        return norm(lowpass(s, 4000), 0.5)
    if kind == "magic_summon":
        n = int(1.5 * RATE)
        t = np.arange(n) / RATE
        whoosh = bandpass(noise(n), 300 + 2200 * (t / 1.5) ** 2, 1.5) * np.sin(np.pi * np.minimum(t / 1.1, 1)) ** 2
        s = whoosh * 0.6
        boom = lowpass(lowpass(noise(int(0.6 * RATE)), 150), 150) * env(int(0.6 * RATE), 0.005, 0.15)
        place(s, norm(boom, 0.9), int(1.0 * RATE))
        return norm(s, 0.6)
    raise SystemExit(f"no magic sound {kind}")


STEPS2 = ["sand", "gravel", "water", "mud", "ice"]
UI = ["ui_move", "ui_confirm", "ui_back", "ui_error", "coin", "quest_done", "page"]
MAGIC = ["magic_frost", "magic_thunder", "magic_heal", "magic_dark", "magic_force", "magic_charm", "magic_acid", "magic_ward", "magic_summon"]


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    only = set(sys.argv[2:])
    want = lambda name: not only or name in only
    for name, fn in [("amb_birds", birds), ("amb_river", river), ("amb_tavern", tavern), ("amb_wind", wind), ("amb_drips", drips)]:
        if not only:
            write_ogg(os.path.join(OUT, name + ".ogg"), fn())
            print(name)
    for kind in ["grass", "dirt", "stone", "wood", "snow"]:
        if not only:
            write_wav(os.path.join(OUT, f"step_{kind}.wav"), step(kind))
            print("step", kind)
    # (Always made in this order, so each keeps its random numbers whatever is asked for.)
    made = [(f"step_{k}", step2(k)) for k in STEPS2] + [(k, ui(k)) for k in UI] + [(k, magic(k)) for k in MAGIC]
    for name, sig in made:
        if want(name):
            write_wav(os.path.join(OUT, name + ".wav"), sig)
            print(name)
