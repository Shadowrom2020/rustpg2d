"""Ambience loops and footsteps for R(ust)PG2D's example (synthesized, mono 44.1 kHz).

Usage: python3 scripts/sounds/make_sounds.py OUT_DIR (numpy; ffmpeg with libvorbis for the loops).

Loops are written as OGG Vorbis (ffmpeg), steps as WAV. Loops end by crossfading their tail
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


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, fn in [("amb_birds", birds), ("amb_river", river), ("amb_tavern", tavern), ("amb_wind", wind), ("amb_drips", drips)]:
        write_ogg(os.path.join(OUT, name + ".ogg"), fn())
        print(name)
    for kind in ["grass", "dirt", "stone", "wood", "snow"]:
        write_wav(os.path.join(OUT, f"step_{kind}.wav"), step(kind))
        print("step", kind)
