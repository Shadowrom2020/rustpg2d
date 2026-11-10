// R(ust)PG2D's plugin for macroquad's JS loader (load it after mq_js_bundle.js, before
// load(...)): the browser storage, clock and gamepads the web game uses (engine/src/sys.rs). Files the
// game writes (save games, settings, achievements) are kept in localStorage under
// "rustpg2d:<page folder>:<path>" as base64 text, so each game on a site has its own.
"use strict";
(function () {
    const prefix = "rustpg2d:" + location.pathname.replace(/[^/]*$/, "") + ":";
    const decoder = new TextDecoder();
    const encoder = new TextEncoder();
    const text = (ptr, len) => decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len));
    let pending = null;
    const take = (dest) => {
        new Uint8Array(wasm_memory.buffer, dest, pending.length).set(pending);
        pending = null;
    };
    miniquad_add_plugin({
        name: "rustpg2d",
        version: 2,
        register_plugin: function (importObject) {
            const env = importObject.env;
            env.rustpg2d_now = () => Date.now() / 1000;
            env.rustpg2d_storage_set = (kp, kl, vp, vl) => {
                try {
                    localStorage.setItem(prefix + text(kp, kl), text(vp, vl));
                } catch (e) {
                    console.error("R(ust)PG2D: could not store " + text(kp, kl) + " (" + e + ")");
                }
            };
            env.rustpg2d_storage_remove = (kp, kl) => {
                try {
                    localStorage.removeItem(prefix + text(kp, kl));
                } catch (e) {}
            };
            env.rustpg2d_storage_dump = () => {
                let s = "";
                try {
                    for (let i = 0; i < localStorage.length; i++) {
                        const k = localStorage.key(i);
                        if (k.startsWith(prefix)) {
                            s += k.slice(prefix.length) + "\t" + localStorage.getItem(k) + "\n";
                        }
                    }
                } catch (e) {
                    console.warn("R(ust)PG2D: no browser storage, nothing will be saved (" + e + ")");
                }
                pending = encoder.encode(s);
                return pending.length;
            };
            env.rustpg2d_storage_take = take;
            env.rustpg2d_query_dump = () => {
                pending = encoder.encode(decodeURIComponent(location.search.replace(/^\?/, "").replace(/\+/g, " ")));
                return pending.length;
            };
            env.rustpg2d_query_take = take;
            env.rustpg2d_language_dump = () => {
                pending = encoder.encode(navigator.language || "");
                return pending.length;
            };
            // Gamepads (the browser's Gamepad API; a pad shows up after its first button press):
            // the first connected one's buttons in the "standard" layout as bits 0-15 (A B X Y,
            // LB RB LT RT, View Menu, stick clicks, d-pad), its stick axes and its name.
            const pad = () => {
                const pads = navigator.getGamepads ? navigator.getGamepads() : [];
                for (const p of pads) {
                    if (p && p.connected) {
                        return p;
                    }
                }
                return null;
            };
            env.rustpg2d_pad_buttons = () => {
                const p = pad();
                let bits = 0;
                if (p) {
                    for (let i = 0; i < Math.min(p.buttons.length, 16); i++) {
                        const b = p.buttons[i];
                        if (b && (b.pressed || b.value > 0.5)) {
                            bits |= 1 << i;
                        }
                    }
                }
                return bits;
            };
            env.rustpg2d_pad_axis = (i) => {
                const p = pad();
                return p && i < p.axes.length ? p.axes[i] : 0;
            };
            env.rustpg2d_pad_id_dump = () => {
                const p = pad();
                pending = encoder.encode(p ? p.id : "");
                return pending.length;
            };
            env.rustpg2d_pad_id_take = take;
        },
    });
})();
